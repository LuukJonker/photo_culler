use byteorder::{BigEndian, ByteOrder, LittleEndian};
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::Path;


/// An embedded JPEG in a RAW file.
#[derive(Default, Eq, PartialEq)]
struct EmbeddedJpegInfo {
    offset: usize,
    length: usize,
    orientation: Option<u16>,
}

fn find_largest_embedded_jpeg_impl<B: ByteOrder>(
    raw_buf: &[u8]
) -> Result<EmbeddedJpegInfo, Box<dyn Error>> {
    const IFD_ENTRY_SIZE: usize = 12;
    const JPEG_TAG: u16 = 0x201;
    const JPEG_LENGTH_TAG: u16 = 0x202;
    const ORIENTATION_TAG: u16 = 0x112;

    let mut next_ifd_offset = B::read_u32(&raw_buf[4..8]).try_into()?;
    let mut largest_jpeg = EmbeddedJpegInfo::default();

    while next_ifd_offset != 0 {
        // ensure!(next_ifd_offset + 2 <= raw_buf.len(), "Invalid IFD offset");

        let cursor = &raw_buf[next_ifd_offset..];
        let num_entries = B::read_u16(&cursor[..2]).into();
        let entries_cursor = &cursor[2..];

        let entries_len = num_entries * IFD_ENTRY_SIZE;
        // ensure!(
        //     entries_cursor.len() >= entries_len,
        //     "Invalid number of IFD entries"
        // );

        let mut cur_offset = None;
        let mut cur_length = None;
        let mut cur_orientation = None;

        for entry in entries_cursor
            .chunks_exact(IFD_ENTRY_SIZE)
            .take(num_entries)
        {
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

        let next_ifd_offset_offset = 2 + entries_len;
        // ensure!(
        //     cursor.len() >= next_ifd_offset_offset + 4,
        //     "Invalid next IFD offset"
        // );
        next_ifd_offset = B::read_u32(&cursor[next_ifd_offset_offset..][..4]).try_into()?;
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
fn find_largest_embedded_jpeg(raw_buf: &[u8]) -> Result<EmbeddedJpegInfo, Box<dyn Error>> {
    const TIFF_MAGIC_LE: &[u8] = b"II*\0";

    // ensure!(raw_buf.len() >= 8, "Not enough data for TIFF header");

    let is_le = &raw_buf[0..4] == TIFF_MAGIC_LE;
    // ensure!(
    //     is_le || &raw_buf[0..4] == TIFF_MAGIC_BE,
    //     "Not a valid TIFF file"
    // );

    let largest_jpeg = if is_le {
        find_largest_embedded_jpeg_impl::<LittleEndian>(raw_buf)?
    } else {
        find_largest_embedded_jpeg_impl::<BigEndian>(raw_buf)?
    };

    // ensure!(
    //     largest_jpeg != EmbeddedJpegInfo::default(),
    //     "No JPEG data found"
    // );
    // ensure!(
    //     largest_jpeg.offset + largest_jpeg.length <= raw_buf.len(),
    //     "JPEG data exceeds file size"
    // );

    Ok(largest_jpeg)
}

/// The embedded JPEG comes with no EXIF data. While most of it is outside of the scope of this
/// application, it's pretty vexing to have the wrong orientation, so copy that over.
#[rustfmt::skip]
const fn get_header_bytes(orientation: u16) -> [u8; 34] {
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
fn extract_jpeg(
    raw_buf: &[u8],
    jpeg: &EmbeddedJpegInfo,
) -> Vec<u8> {
    // Look later if actually fast
    let mut hdr_bytes = get_header_bytes(jpeg.orientation.unwrap_or(1)).to_vec();
    hdr_bytes.extend(raw_buf[jpeg.offset..jpeg.offset + jpeg.length].to_vec());
    raw_buf[jpeg.offset..jpeg.offset + jpeg.length].to_vec()
}

/// Process a single RAW file to extract the embedded JPEG, and then write the extracted JPEG to
/// the output directory.
pub fn process_file(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut in_file = File::open(path)?;
    let mut raw_buf = Vec::new();
    in_file.read_to_end(&mut raw_buf)?;

    let jpeg_info = find_largest_embedded_jpeg(&raw_buf)?;
    let jpeg_buf = extract_jpeg(&raw_buf, &jpeg_info);

    Ok(jpeg_buf)
}
