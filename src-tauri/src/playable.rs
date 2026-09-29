use std::path::Path;

const BROKEN_AUDIO: u32 = 4;

#[cfg(windows)]
pub fn remux(src: &Path, dst: &Path) -> windows::core::Result<usize> {
    use windows::core::HSTRING;
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let _ = MFStartup(MF_VERSION, MFSTARTUP_FULL);
        let reader = MFCreateSourceReaderFromURL(&HSTRING::from(src.as_os_str()), None)?;
        let part = dst.with_extension("part.mp4");
        let _ = std::fs::remove_file(&part);
        let writer = MFCreateSinkWriterFromURL(&HSTRING::from(part.as_os_str()), None, None)?;
        let mut outs: Vec<Option<(u32, bool)>> = Vec::new();
        let mut stream = 0u32;
        while let Ok(native) = reader.GetNativeMediaType(stream, 0) {
            let major = native.GetMajorType()?;
            if major == MFMediaType_Video || major == MFMediaType_Audio {
                reader.SetStreamSelection(stream, true)?;
                let out = writer.AddStream(&native)?;
                writer.SetInputMediaType(out, &native, None)?;
                outs.push(Some((out, major == MFMediaType_Audio)));
            } else {
                reader.SetStreamSelection(stream, false)?;
                outs.push(None);
            }
            stream += 1;
        }
        writer.BeginWriting()?;
        let mut dropped = 0;
        loop {
            let (mut index, mut flags, mut ts, mut sample) = (0u32, 0u32, 0i64, None);
            reader.ReadSample(MF_SOURCE_READER_ANY_STREAM.0 as u32, 0, Some(&mut index), Some(&mut flags), Some(&mut ts), Some(&mut sample))?;
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                if let Some(Some((out, _))) = outs.get(index as usize) {
                    let _ = writer.NotifyEndOfSegment(*out);
                }
                outs[index as usize] = None;
                if outs.iter().all(Option::is_none) {
                    break;
                }
                continue;
            }
            let (Some(sample), Some(Some((out, audio)))) = (sample, outs.get(index as usize)) else { continue };
            if *audio && sample.GetTotalLength()? < BROKEN_AUDIO {
                dropped += 1;
                continue;
            }
            writer.WriteSample(*out, &sample)?;
        }
        writer.Finalize()?;
        std::fs::rename(&part, dst).map_err(|e| windows::core::Error::new(windows::core::HRESULT(-1), e.to_string()))?;
        Ok(dropped)
    }
}

#[cfg(not(windows))]
pub fn remux(_: &Path, _: &Path) -> std::io::Result<usize> {
    Err(std::io::Error::other("только Windows"))
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    #[ignore]
    fn fixes_broken_first_audio_packet() {
        let src = std::path::Path::new(r"G:\sorter-test\deck\VID_131650126_113909_242.mp4");
        if !src.exists() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("play.mp4");
        let dropped = super::remux(src, &dst).unwrap();
        assert_eq!(dropped, 1);
        assert!(std::fs::metadata(&dst).unwrap().len() > 1_000_000);
    }
}
