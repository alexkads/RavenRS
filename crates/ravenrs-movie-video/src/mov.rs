//! A QuickTime file with one H.264 picture track and one track of plain sound, the
//! simplest container that video sites, players and editors all read.

use std::io::{self, Seek, SeekFrom, Write};

/// One picture, as length-prefixed NAL units.
pub(crate) struct Picture {
    pub(crate) offset: u64,
    pub(crate) size: u32,
    pub(crate) key: bool,
}

/// Writes the pictures and the sound as they come, and the index at the end.
pub(crate) struct MovWriter<W: Write + Seek> {
    out: W,
    mdat_start: u64,
    pictures: Vec<Picture>,
    /// Where each piece of sound went and how many samples it holds.
    sound: Vec<(u64, u32)>,
    sps: Vec<u8>,
    pps: Vec<u8>,
}

impl<W: Write + Seek> MovWriter<W> {
    pub(crate) fn new(mut out: W) -> io::Result<Self> {
        let mut ftyp = Vec::new();
        ftyp.extend_from_slice(b"qt  ");
        ftyp.extend_from_slice(&0x2005_0300_u32.to_be_bytes());
        ftyp.extend_from_slice(b"qt  ");
        write_box(&mut out, b"ftyp", &ftyp)?;
        let mdat_start = out.stream_position()?;
        // A 64-bit size, filled in at the end.
        out.write_all(&1_u32.to_be_bytes())?;
        out.write_all(b"mdat")?;
        out.write_all(&0_u64.to_be_bytes())?;
        Ok(Self {
            out,
            mdat_start,
            pictures: Vec::new(),
            sound: Vec::new(),
            sps: Vec::new(),
            pps: Vec::new(),
        })
    }

    /// Adds a picture from its NAL units, without their start codes. The parameter
    /// sets go to the index, the rest to the file.
    pub(crate) fn picture(&mut self, nals: &[&[u8]]) -> io::Result<()> {
        let offset = self.out.stream_position()?;
        let (mut size, mut key) = (0_u32, false);
        for nal in nals {
            let Some(header) = nal.first() else { continue };
            match header & 0x1F {
                7 => self.sps = nal.to_vec(),
                8 => self.pps = nal.to_vec(),
                kind => {
                    key |= kind == 5;
                    self.out.write_all(&(nal.len() as u32).to_be_bytes())?;
                    self.out.write_all(nal)?;
                    size += 4 + nal.len() as u32;
                }
            }
        }
        self.pictures.push(Picture { offset, size, key });
        Ok(())
    }

    /// Adds a piece of sound, 16-bit little endian, one channel.
    pub(crate) fn sound(&mut self, samples: &[i16]) -> io::Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        let offset = self.out.stream_position()?;
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        self.out.write_all(&bytes)?;
        self.sound.push((offset, samples.len() as u32));
        Ok(())
    }

    /// Writes the index: `size` of the pictures, `fps` of them a second, sound at `rate`.
    pub(crate) fn finish(mut self, size: (u32, u32), fps: u32, rate: u32) -> io::Result<W> {
        let end = self.out.stream_position()?;
        self.out.seek(SeekFrom::Start(self.mdat_start + 8))?;
        self.out.write_all(&(end - self.mdat_start).to_be_bytes())?;
        self.out.seek(SeekFrom::Start(end))?;
        let frames = self.pictures.len() as u64;
        let sound_samples: u64 = self.sound.iter().map(|(_, n)| u64::from(*n)).sum();
        let movie_ms = (frames * 1000).div_ceil(u64::from(fps.max(1)));
        let mut moov = Vec::new();
        moov.extend(boxed(b"mvhd", &mvhd(movie_ms, 3)));
        moov.extend(self.video_track(size, fps, movie_ms));
        if sound_samples > 0 {
            moov.extend(self.sound_track(rate, sound_samples));
        }
        write_box(&mut self.out, b"moov", &moov)?;
        Ok(self.out)
    }

    fn video_track(&self, (width, height): (u32, u32), fps: u32, movie_ms: u64) -> Vec<u8> {
        let scale = fps * 100;
        let frames = self.pictures.len() as u32;
        let mut entry = Vec::new();
        entry.extend_from_slice(&[0; 6]);
        entry.extend_from_slice(&1_u16.to_be_bytes());
        entry.extend_from_slice(&[0; 16]);
        entry.extend_from_slice(&(width as u16).to_be_bytes());
        entry.extend_from_slice(&(height as u16).to_be_bytes());
        entry.extend_from_slice(&0x0048_0000_u32.to_be_bytes());
        entry.extend_from_slice(&0x0048_0000_u32.to_be_bytes());
        entry.extend_from_slice(&0_u32.to_be_bytes());
        entry.extend_from_slice(&1_u16.to_be_bytes());
        let mut name = [0_u8; 32];
        let label = b"H.264";
        name[0] = label.len() as u8;
        name[1..=label.len()].copy_from_slice(label);
        entry.extend_from_slice(&name);
        entry.extend_from_slice(&24_u16.to_be_bytes());
        entry.extend_from_slice(&(-1_i16).to_be_bytes());
        entry.extend(boxed(b"avcC", &self.avcc()));
        let stsd = full(0, 0, &[&1_u32.to_be_bytes()[..], &boxed(b"avc1", &entry)].concat());
        let stts = full(0, 0, &[1_u32.to_be_bytes(), frames.to_be_bytes(), 100_u32.to_be_bytes()].concat());
        let keys: Vec<u32> =
            self.pictures.iter().enumerate().filter(|(_, p)| p.key).map(|(i, _)| i as u32 + 1).collect();
        let mut stss = (keys.len() as u32).to_be_bytes().to_vec();
        keys.iter().for_each(|k| stss.extend_from_slice(&k.to_be_bytes()));
        let stsc = full(
            0,
            0,
            &[1_u32.to_be_bytes(), 1_u32.to_be_bytes(), 1_u32.to_be_bytes(), 1_u32.to_be_bytes()].concat(),
        );
        let mut stsz = [0_u32.to_be_bytes(), frames.to_be_bytes()].concat();
        self.pictures.iter().for_each(|p| stsz.extend_from_slice(&p.size.to_be_bytes()));
        let offsets: Vec<u64> = self.pictures.iter().map(|p| p.offset).collect();
        let stbl = [
            boxed(b"stsd", &stsd),
            boxed(b"stts", &stts),
            boxed(b"stss", &full(0, 0, &stss)),
            boxed(b"stsc", &stsc),
            boxed(b"stsz", &full(0, 0, &stsz)),
            chunk_offsets(&offsets),
        ]
        .concat();
        let vmhd = boxed(b"vmhd", &full(0, 1, &[0_u8; 8]));
        let duration = u64::from(frames) * 100;
        track(1, (width, height), movie_ms, scale, duration, b"vide", vmhd, stbl)
    }

    fn sound_track(&self, rate: u32, samples: u64) -> Vec<u8> {
        let mut entry = Vec::new();
        entry.extend_from_slice(&[0; 6]);
        entry.extend_from_slice(&1_u16.to_be_bytes());
        // Version 0 of the sound description: one channel of 16 bits.
        entry.extend_from_slice(&[0; 8]);
        entry.extend_from_slice(&1_u16.to_be_bytes());
        entry.extend_from_slice(&16_u16.to_be_bytes());
        entry.extend_from_slice(&0_u16.to_be_bytes());
        entry.extend_from_slice(&0_u16.to_be_bytes());
        entry.extend_from_slice(&(rate << 16).to_be_bytes());
        let stsd = full(0, 0, &[&1_u32.to_be_bytes()[..], &boxed(b"sowt", &entry)].concat());
        let stts =
            full(0, 0, &[1_u32.to_be_bytes(), (samples as u32).to_be_bytes(), 1_u32.to_be_bytes()].concat());
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for (index, (_, count)) in self.sound.iter().enumerate() {
            if runs.last().is_none_or(|(_, last)| last != count) {
                runs.push((index as u32 + 1, *count));
            }
        }
        let mut stsc = (runs.len() as u32).to_be_bytes().to_vec();
        for (first, count) in runs {
            stsc.extend_from_slice(&first.to_be_bytes());
            stsc.extend_from_slice(&count.to_be_bytes());
            stsc.extend_from_slice(&1_u32.to_be_bytes());
        }
        let stsz = full(0, 0, &[2_u32.to_be_bytes(), (samples as u32).to_be_bytes()].concat());
        let offsets: Vec<u64> = self.sound.iter().map(|(o, _)| *o).collect();
        let stbl = [
            boxed(b"stsd", &stsd),
            boxed(b"stts", &stts),
            boxed(b"stsc", &full(0, 0, &stsc)),
            boxed(b"stsz", &stsz),
            chunk_offsets(&offsets),
        ]
        .concat();
        let smhd = boxed(b"smhd", &full(0, 0, &[0_u8; 4]));
        let movie_ms = samples * 1000 / u64::from(rate.max(1));
        track(2, (0, 0), movie_ms, rate, samples, b"soun", smhd, stbl)
    }

    fn avcc(&self) -> Vec<u8> {
        let sps = &self.sps;
        let mut out = vec![
            1,
            *sps.get(1).unwrap_or(&66),
            *sps.get(2).unwrap_or(&0),
            *sps.get(3).unwrap_or(&40),
            0xFF,
            0xE1,
        ];
        out.extend_from_slice(&(sps.len() as u16).to_be_bytes());
        out.extend_from_slice(sps);
        out.push(1);
        out.extend_from_slice(&(self.pps.len() as u16).to_be_bytes());
        out.extend_from_slice(&self.pps);
        out
    }
}

fn chunk_offsets(offsets: &[u64]) -> Vec<u8> {
    let large = offsets.iter().any(|o| *o > u64::from(u32::MAX));
    let mut body = (offsets.len() as u32).to_be_bytes().to_vec();
    for offset in offsets {
        if large {
            body.extend_from_slice(&offset.to_be_bytes());
        } else {
            body.extend_from_slice(&(*offset as u32).to_be_bytes());
        }
    }
    boxed(if large { b"co64" } else { b"stco" }, &full(0, 0, &body))
}

#[allow(clippy::too_many_arguments, reason = "the fields of one track")]
fn track(
    id: u32,
    (width, height): (u32, u32),
    movie_ms: u64,
    scale: u32,
    duration: u64,
    kind: &[u8; 4],
    header: Vec<u8>,
    stbl: Vec<u8>,
) -> Vec<u8> {
    let mut tkhd = Vec::new();
    tkhd.extend_from_slice(&[0; 8]);
    tkhd.extend_from_slice(&id.to_be_bytes());
    tkhd.extend_from_slice(&[0; 4]);
    tkhd.extend_from_slice(&(movie_ms as u32).to_be_bytes());
    tkhd.extend_from_slice(&[0; 8]);
    tkhd.extend_from_slice(&0_u16.to_be_bytes());
    tkhd.extend_from_slice(&0_u16.to_be_bytes());
    tkhd.extend_from_slice(&(if kind == b"soun" { 0x0100_u16 } else { 0 }).to_be_bytes());
    tkhd.extend_from_slice(&[0; 2]);
    tkhd.extend(matrix());
    tkhd.extend_from_slice(&(width << 16).to_be_bytes());
    tkhd.extend_from_slice(&(height << 16).to_be_bytes());
    let mut mdhd = Vec::new();
    mdhd.extend_from_slice(&[0; 8]);
    mdhd.extend_from_slice(&scale.to_be_bytes());
    mdhd.extend_from_slice(&(duration as u32).to_be_bytes());
    mdhd.extend_from_slice(&0_u16.to_be_bytes());
    mdhd.extend_from_slice(&0_u16.to_be_bytes());
    let handler = |class: &[u8; 4], sub: &[u8; 4], name: &str| {
        let mut body = Vec::new();
        body.extend_from_slice(class);
        body.extend_from_slice(sub);
        body.extend_from_slice(&[0; 12]);
        body.push(name.len() as u8);
        body.extend_from_slice(name.as_bytes());
        boxed(b"hdlr", &full(0, 0, &body))
    };
    let dref = full(0, 0, &[&1_u32.to_be_bytes()[..], &boxed(b"alis", &full(0, 1, &[]))].concat());
    let dinf = boxed(b"dinf", &boxed(b"dref", &dref));
    let minf = [header, handler(b"dhlr", b"alis", "RavenRS"), dinf, boxed(b"stbl", &stbl)].concat();
    let mdia = [boxed(b"mdhd", &full(0, 0, &mdhd)), handler(b"mhlr", kind, "RavenRS"), boxed(b"minf", &minf)]
        .concat();
    boxed(b"trak", &[boxed(b"tkhd", &full(0, 0x3, &tkhd)), boxed(b"mdia", &mdia)].concat())
}

fn mvhd(duration_ms: u64, next_track: u32) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&[0; 8]);
    body.extend_from_slice(&1000_u32.to_be_bytes());
    body.extend_from_slice(&(duration_ms as u32).to_be_bytes());
    body.extend_from_slice(&0x0001_0000_u32.to_be_bytes());
    body.extend_from_slice(&0x0100_u16.to_be_bytes());
    body.extend_from_slice(&[0; 10]);
    body.extend(matrix());
    body.extend_from_slice(&[0; 24]);
    body.extend_from_slice(&next_track.to_be_bytes());
    full(0, 0, &body)
}

fn matrix() -> Vec<u8> {
    [0x0001_0000_u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000]
        .iter()
        .flat_map(|v| v.to_be_bytes())
        .collect()
}

fn full(version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let mut out = vec![version];
    out.extend_from_slice(&flags.to_be_bytes()[1..]);
    out.extend_from_slice(body);
    out
}

fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out
}

fn write_box(out: &mut impl Write, kind: &[u8; 4], body: &[u8]) -> io::Result<()> {
    out.write_all(&boxed(kind, body))
}
