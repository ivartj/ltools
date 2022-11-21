use std::io::{
    Write,
    Result,
};

struct Location {
    line: usize,
    column: usize,
}

impl Location {
    pub fn new() -> Location {
        Location{
            line: 0,
            column: 0,
        }
    }

    pub fn after(self, c: u8) -> Self {
        match c {
            b'\n' => Location {
                line: self.line + 1,
                column: 0,
            },
            _ => Location {
                line: self.line + 1,
                column: 0,
            },
        }
    }
}

struct Segment {
    location: Location,
    data: &[u8],
}

pub trait WriteSegment {
    fn write_segment(&mut self, seg: Segment) -> Result<usize>;
}

pub struct SegmentWriter<WS: WriteSegment> {
    location: Location,
    inner: WS,
}

impl SegmentWriter<WS: WriteSegment> {
    fn new(inner: WS) -> SegmentWriter<WS> {
        SegmentWriter{
            location: Location::new(),
            inner,
        }
    }
}

impl Write for SegmentWriter<WS: WriteSegment> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let seg = Segment{
            location: self.location,
            buf,
        };
        let newlines_indices = buf.copied()
            .enumerate()
            .filter(|(_, c)| c == b'\n')
            .map(|(i, _)| i)
            .collect();
        self.line += newlines.len();
        if let Some(last_newline_index) = newline_indices.last() {
            self.column = 1 + buf.len() - last_newline_index;
        } else {
            self.column += buf.len();
        }
        self.inner.write_segment(seg)?;
    }
}

