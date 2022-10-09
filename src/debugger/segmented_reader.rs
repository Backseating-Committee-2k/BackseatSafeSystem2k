use std::{
    cmp::max,
    fmt,
    io::{self, Read},
    ops::Range,
};

const INITIAL_BUFFER_SIZE: usize = 1024;

pub struct SegmentedReader {
    buffer: Vec<u8>,
    length: usize,
    next_segment_start: usize,
    buffer_version: u32,
}

#[derive(Debug, PartialEq)]
pub struct Segment {
    range: Range<usize>,
    buffer_version: u32,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Disconnected,
}

pub type Result<T> = std::result::Result<T, Error>;

impl SegmentedReader {
    pub fn new() -> Self {
        Self {
            buffer: vec![0; INITIAL_BUFFER_SIZE],
            length: 0,
            buffer_version: 0,
            next_segment_start: 0,
        }
    }

    pub fn read(&mut self, mut from: impl Read) -> Result<Vec<Segment>> {
        self.consume_old_segments();
        if self.length == self.buffer.len() {
            self.grow_buffer();
        }

        let length = from
            .read(&mut self.buffer[self.length..])
            .map_err(Error::Io)?;

        if length == 0 {
            return Err(Error::Disconnected);
        }

        let (search_start, search_end) = (self.length, self.length + length);
        self.length += length;

        let endings = self.buffer[search_start..search_end]
            .iter()
            .enumerate()
            .filter(|(_, &byte)| byte == 0)
            .map(|(index, _)| index + search_start);

        let mut segments = Vec::new();
        for ending in endings {
            segments.push(Segment {
                range: self.next_segment_start..ending,
                buffer_version: self.buffer_version,
            });
            self.next_segment_start = ending + 1;
        }

        Ok(segments)
    }

    pub fn segment(&self, s: &Segment) -> &[u8] {
        if s.buffer_version != self.buffer_version {
            panic!("Cannot access old segment after new data is read into buffer");
        }
        &self.buffer[s.range.clone()]
    }

    pub fn clear(&mut self) {
        self.buffer_version += 1;
        self.length = 0;
        self.next_segment_start = 0;
    }

    fn consume_old_segments(&mut self) {
        if self.next_segment_start == 0 {
            return;
        }

        self.buffer_version += 1; // Invalidate old segments

        if self.next_segment_start == self.length {
            self.length = 0;
            self.next_segment_start = 0;
        } else if self.next_segment_start != 0 {
            let buffer_size = max(
                INITIAL_BUFFER_SIZE,
                2 * (self.length - self.next_segment_start),
            );
            let mut new_buffer = vec![0; buffer_size];
            new_buffer[0..(self.length - self.next_segment_start)]
                .copy_from_slice(&self.buffer[self.next_segment_start..self.length]);

            self.buffer = new_buffer;
            self.length = self.length - self.next_segment_start;
            self.next_segment_start = 0;
        }
    }

    fn grow_buffer(&mut self) {
        let new_size = 2 * self.buffer.len();
        self.buffer.resize(new_size, 0);
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(error) => error.fmt(f),
            Error::Disconnected => write!(f, "input disconnected"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_returns_correct_segments() -> Result<()> {
        let mut reader = SegmentedReader::new();
        let read = b"hello\0world\0";
        let segments = reader.read(&read[..])?;
        assert_eq!(
            &[
                Segment {
                    range: 0..5,
                    buffer_version: reader.buffer_version
                },
                Segment {
                    range: 6..11,
                    buffer_version: reader.buffer_version
                }
            ][..],
            &segments[..]
        );
        Ok(())
    }

    #[test]
    fn read_returns_disconnected_when_reading_zero_bytes() {
        let mut reader = SegmentedReader::new();
        let result = reader.read(&b""[..]);
        assert!(matches!(result, Err(Error::Disconnected)));
    }

    #[test]
    fn segment_returns_correct_slice() -> Result<()> {
        let mut reader = SegmentedReader::new();
        let read = b"hello\0world\0";
        let segments = reader.read(&read[..])?;
        assert_eq!(2, segments.len());
        assert_eq!(&b"hello"[..], reader.segment(&segments[0]));
        assert_eq!(&b"world"[..], reader.segment(&segments[1]));

        let mut reader = SegmentedReader::new();
        let read = b"\0this is some text\0012345\0\0not finished yet...";
        let segments = reader.read(&read[..])?;
        assert_eq!(4, segments.len());
        assert_eq!(&b""[..], reader.segment(&segments[0]));
        assert_eq!(&b"this is some text"[..], reader.segment(&segments[1]));
        assert_eq!(&b"012345"[..], reader.segment(&segments[2]));
        assert_eq!(&b""[..], reader.segment(&segments[3]));

        Ok(())
    }

    #[test]
    fn segment_panics_when_given_invalid_segment() -> Result<()> {
        let mut reader = SegmentedReader::new();
        let read = b"hello\0world\0";
        let mut segments = reader.read(&read[..])?;

        segments[0].buffer_version += 42;
        let panic = std::panic::catch_unwind(|| reader.segment(&segments[0]));
        assert!(panic.is_err());

        Ok(())
    }

    #[test]
    fn multiple_reads_work_correctly() -> Result<()> {
        let mut reader = SegmentedReader::new();
        let read1 = b"hello\0world\0";
        let read2 = b"simple\0case\0";
        reader.read(&read1[..])?;
        let segments2 = reader.read(&read2[..])?;

        assert_eq!(2, segments2.len());
        assert_eq!(&b"simple"[..], reader.segment(&segments2[0]));
        assert_eq!(&b"case"[..], reader.segment(&segments2[1]));

        Ok(())
    }

    #[test]
    fn read_invalidates_old_segments() -> Result<()> {
        let mut reader = SegmentedReader::new();
        let read1 = b"hello\0world\0";
        let read2 = b"simple\0case\0";
        let segments1 = reader.read(&read1[..])?;
        reader.read(&read2[..])?;

        assert_eq!(2, segments1.len());
        let panic = std::panic::catch_unwind(|| reader.segment(&segments1[0]));
        assert!(panic.is_err());

        Ok(())
    }

    #[test]
    fn read_overlapping_segments() -> Result<()> {
        let mut reader = SegmentedReader::new();
        let read1 = b"hello\0world\0over";
        let read2 = b"lapping\0case\0";

        let segments1 = reader.read(&read1[..])?;
        assert_eq!(2, segments1.len());
        assert_eq!(&b"hello"[..], reader.segment(&segments1[0]));
        assert_eq!(&b"world"[..], reader.segment(&segments1[1]));

        let segments2 = reader.read(&read2[..])?;
        assert_eq!(2, segments2.len());
        assert_eq!(&b"overlapping"[..], reader.segment(&segments2[0]));
        assert_eq!(&b"case"[..], reader.segment(&segments2[1]));

        Ok(())
    }

    #[test]
    fn multiple_reads_for_single_segment() -> Result<()> {
        let mut reader = SegmentedReader::new();
        let read1 = b"read1 ";
        let read2 = b"read2 ";
        let read3 = b"read3\0";

        let segments = reader.read(&read1[..])?;
        assert_eq!(0, segments.len());
        let segments = reader.read(&read2[..])?;
        assert_eq!(0, segments.len());
        let segments = reader.read(&read3[..])?;
        assert_eq!(1, segments.len());
        assert_eq!(&b"read1 read2 read3"[..], reader.segment(&segments[0]));

        Ok(())
    }

    #[test]
    fn read_grows_buffer() -> Result<()> {
        let mut reader = SegmentedReader::new();

        let buffer_size = reader.buffer.len();
        let mut read = vec![5; 3 * buffer_size];
        *read.last_mut().unwrap() = 0;

        let mut slice = &read[..];
        let segments = loop {
            let segments = reader.read(&mut slice)?;
            if segments.len() != 0 {
                break segments;
            }
        };

        assert_eq!(1, segments.len());
        assert_eq!(&read[0..3 * buffer_size - 1], reader.segment(&segments[0]));

        Ok(())
    }

    #[test]
    fn clear_resets_reader_correctly() -> Result<()> {
        let mut reader = SegmentedReader::new();

        let read1 = b"read1";
        let segments = reader.read(&read1[..])?;
        assert_eq!(0, segments.len());

        let read2 = b"read2\0";
        reader.clear();
        let segments = reader.read(&read2[..])?;
        assert_eq!(1, segments.len());
        assert_eq!(&b"read2"[..], reader.segment(&segments[0]));

        Ok(())
    }

    #[test]
    fn clear_invalidates_old_segments() -> Result<()> {
        let mut reader = SegmentedReader::new();

        let read1 = b"read1\0read2";
        let segments = reader.read(&read1[..])?;
        assert_eq!(1, segments.len());

        reader.clear();
        let panic = std::panic::catch_unwind(|| reader.segment(&segments[0]));
        assert!(panic.is_err());

        Ok(())
    }
}
