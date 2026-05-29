use byteorder::{BigEndian, ByteOrder, LittleEndian};
use std::error::Error;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::ModelError;

const HEADER_LENGTH: usize = 34;

/// An embedded JPEG in a RAW file.
#[derive(Default, Eq, PartialEq)]
struct EmbeddedJpegInfo {
    offset: usize,
    length: usize,
    orientation: Option<u16>,
}

fn find_largest_embedded_jpeg_impl<B: ByteOrder>(
    file: &mut File,
) -> Result<EmbeddedJpegInfo, Box<dyn Error>> {
    const IFD_ENTRY_SIZE: usize = 12;
    const JPEG_TAG: u16 = 0x201;
    const JPEG_LENGTH_TAG: u16 = 0x202;
    const ORIENTATION_TAG: u16 = 0x112;

    // Read in the offset from the file. We need to skip 4 bytes, but did that
    // in the caller of this function.
    let mut offset_buf = [0; 4];
    file.read_exact(&mut offset_buf)?;

    // Decode from the buffer
    let mut next_ifd_offset = B::read_u32(&offset_buf).try_into()?;
    let mut largest_jpeg = EmbeddedJpegInfo::default();

    // Allocate the entries buf here and resize in loop to only request new memory if the buf needs to be larger

    while next_ifd_offset != 0 {
        // Get the number of entries from the
        let mut num_entries_buf = [0; 2];
        file.seek(SeekFrom::Start(next_ifd_offset))?;
        if file.read_exact(&mut num_entries_buf).is_err() {
            // We break if the file ended and we can't read no more
            break;
        }

        // Decode the number of entries
        let num_entries = B::read_u16(&num_entries_buf).into();

        // Calculate the length in bytes
        let entries_len = num_entries * IFD_ENTRY_SIZE;

        // Need 6 extra bytes to include the size of the next ifd offset. Also
        let mut entries_buf = vec![0; entries_len + 6];
        file.read_exact(&mut entries_buf)?;

        let mut cur_offset = None;
        let mut cur_length = None;
        let mut cur_orientation = None;

        for entry in entries_buf.chunks_exact(IFD_ENTRY_SIZE).take(num_entries) {
            let tag = B::read_u16(&entry[..2]);

            match tag {
                JPEG_TAG => cur_offset = Some(B::read_u32(&entry[8..12]).try_into()?),
                JPEG_LENGTH_TAG => cur_length = Some(B::read_u32(&entry[8..12]).try_into()?),
                ORIENTATION_TAG => cur_orientation = Some(B::read_u16(&entry[8..10])),
                _ => {}
            }
        }

        if let (Some(offset), Some(length)) = (cur_offset, cur_length) {
            if length > largest_jpeg.length {
                largest_jpeg = EmbeddedJpegInfo {
                    offset,
                    length,
                    orientation: cur_orientation,
                };
            }
        }

        next_ifd_offset = B::read_u32(&entries_buf[2 + entries_len..][..4]).try_into()?;
    }

    // Check if there was actually a jpeg found, otherwise return error
    if largest_jpeg == EmbeddedJpegInfo::default() {
        return Err(Box::new(ModelError::WithMessage(
            "Couldn't find jpeg in raw image".into(),
        )));
    }

    Ok(largest_jpeg)
}

/// Find the largest embedded JPEG data in a memory-mapped RAW buffer.
///
/// This function parses the IFDs in the TIFF structure of the RAW file to find the largest JPEG
/// thumbnail embedded in the file.
///
/// We hand roll the IFD parsing because libraries do not fit requirements. For example:
///
/// - kamadak-exif: Reads into a big `Vec<u8>`, which is huge for our big RAW.
/// - quickexif: Cannot iterate over IFDs.
fn find_largest_embedded_jpeg(file: &mut File) -> Result<EmbeddedJpegInfo, Box<dyn Error>> {
    const TIFF_MAGIC_LE: &[u8] = b"II*\0";

    let mut magic_buf: [u8; 4] = [0; 4];
    file.read_exact(&mut magic_buf)?;
    let is_le = magic_buf == TIFF_MAGIC_LE;

    let largest_jpeg = if is_le {
        find_largest_embedded_jpeg_impl::<LittleEndian>(file)?
    } else {
        find_largest_embedded_jpeg_impl::<BigEndian>(file)?
    };

    Ok(largest_jpeg)
}

/// The embedded JPEG comes with no EXIF data. While most of it is outside of the scope of this
/// application, it's pretty vexing to have the wrong orientation, so copy that over.
#[rustfmt::skip]
const fn get_header_bytes(orientation: u16) -> [u8; HEADER_LENGTH] {
    let orientation_bytes = orientation.to_le_bytes();
    [
        0xff, 0xd8, // SOI
        0xff, 0xe1, // APP1
        0x00, 0x1e, // 30 bytes including this length
        0x45, 0x78, 0x69, 0x66, 0x00, 0x00, // Exif\0\0
        0x49, 0x49, 0x2A, 0x00, // TIFF LE
        0x08, 0x00, 0x00, 0x00, // Offset to IFD
        0x01, 0x00, // One entry in IFD
        0x12, 0x01, // Tag for orientation
        0x03, 0x00, // Type: SHORT
        0x01, 0x00, 0x00, 0x00, // Count: 1
        orientation_bytes[0], orientation_bytes[1], // Orientation
        0x00, 0x00, // Next IFD
    ]
}

/// Extract the JPEG bytes from the memory-mapped RAW buffer.
fn extract_jpeg(file: &mut File, jpeg: &EmbeddedJpegInfo) -> Result<Vec<u8>, Box<dyn Error>> {
    let total_size = HEADER_LENGTH + jpeg.length - 2;

    // Initialize the vector with zeros so `len` == `capacity`
    let mut buf = vec![0u8; total_size];

    // 1. Write the header to the beginning of the slice
    let header = get_header_bytes(jpeg.orientation.unwrap_or(1));
    buf[..HEADER_LENGTH].copy_from_slice(&header);

    // 2. Seek to the start of the payload
    file.seek(SeekFrom::Start(jpeg.offset as u64 + 2))?;

    // 3. Read the file into the remainder of the buffer
    // Now that `buf.len()` is the total size, this slice actually represents
    // the remaining megabytes of memory.
    file.read_exact(&mut buf[HEADER_LENGTH..])?;

    Ok(buf)
}
/// Process a single RAW file to extract the embedded JPEG, and then write the extracted JPEG to
/// the output directory.
pub fn process_file(path: &Path) -> Result<(Vec<u8>, u16), Box<dyn Error>> {
    let mut in_file = File::open(path)?;

    let jpeg_info = find_largest_embedded_jpeg(&mut in_file)?;

    let jpg = extract_jpeg(&mut in_file, &jpeg_info)?;

    Ok((jpg, jpeg_info.orientation.unwrap_or(1)))
}
