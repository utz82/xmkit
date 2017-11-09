/// A module for extracting information from eXtended Module (XM) files.
pub mod xmkit {
    use std::error::Error;
    use std::fmt;
    use std::fs;
    use std::io::prelude::*;
    use std::path::Path;
    use std::str;

    const XM_MODULE_NAME: usize = 0x11;
    const XM_TRACKER_NAME: usize = 0x25;
    const XM_VERSION_MINOR: usize = 0x3a;
    const XM_VERSION_MAJOR: usize = 0x3b;
    const XM_HEADER_SIZE: usize = 0x3c;
    const XM_SEQUENCE_LEN: usize = 0x40;
    const XM_RESTART_POS: usize = 0x42;
    const XM_CHANNEL_COUNT: usize = 0x44;
    const XM_PATTERN_COUNT: usize = 0x46;
    const XM_INSTRUMENT_COUNT: usize = 0x48;
    const XM_FREQ_TABLE_TYPE: usize = 0x4a;
    const XM_DEFAULT_TEMPO: usize = 0x4c;
    const XM_DEFAULT_BPM: usize = 0x4e;    
    const XM_SEQUENCE_BEGIN: usize = 0x50;

    // #[derive(Default, Debug)]
    #[derive(Default)]
    pub struct XModule {
        xmdata: Vec<u8>,
        patterns: Vec<XMPattern>,
        instruments: Vec<XMInstrument>,
    }

    impl XModule {
        // Just performs a call to Default::default(), so you might as well use that directly. 
        // pub fn new() -> XModule {
        //     Default::default()
        // }
        
        // parse should just parse the data, and open() should serve as a wrapper for opening and parsing an xm file
        /// Parses an eXtended Module (XM) file.
        ///
        /// # Panics
        /// May `panic!` if the XM file is modified while running this function.
        pub fn parse(filepath: &Path) -> Result<XModule, XMParseError> {
            let mut xmfile = match fs::File::open(&filepath) {
                // TODO should propagate the actual io::Error instead of converting it
                Err(e) => return Err(XMParseError::new(&format!("Couldn't open {}: {}", filepath.display(), e.description()))),
                Ok(xmfile) => xmfile,
            };

            let metadata = match fs::metadata(&filepath) {
                // TODO should propagate the actual io::Error instead of converting it
                Err(e) => return Err(XMParseError::new(&format!("{}: Couldn't read metadata: {}", 
                    filepath.display(), e.description()))),
                Ok(metadata) => metadata,   
            };

            let mut xm: XModule = Default::default();

            xm.xmdata = Vec::with_capacity(metadata.len() as usize);
            fs::File::read_to_end(&mut xmfile, &mut xm.xmdata).unwrap();
            
            match xm.verify_filetype() {
                Err(e) => return Err(XMParseError::new(&format!("{}: {}", filepath.display(), e))),
                Ok(..) => (),
            };

            // calculate beginning of pattern data; stored header size 
            // does not include bytes up to XM_HEADER_SIZE offset (0x3c)
            let mut file_offset: usize = XM_HEADER_SIZE + xm.read_usize(XM_HEADER_SIZE);

            // parse pattern data
            for _ in 0..xm.pattern_count() {
                let ptn_len = xm.xmdata[file_offset + 5];
                let packed_size = xm.read_u16(file_offset + 7);
                file_offset += xm.read_usize(file_offset);
                let next_offset = file_offset + packed_size as usize;

                let mut notes: Vec<Vec<Option<u8>>> = Vec::with_capacity(xm.channel_count() as usize);
                let mut instruments: Vec<Vec<Option<u8>>> = Vec::with_capacity(xm.channel_count() as usize);
                let mut volumes: Vec<Vec<Option<u8>>> = Vec::with_capacity(xm.channel_count() as usize);
                let mut fx_commands: Vec<Vec<Option<u8>>> = Vec::with_capacity(xm.channel_count() as usize);
                let mut fx_params: Vec<Vec<Option<u8>>> = Vec::with_capacity(xm.channel_count() as usize);

                for _ in 0..xm.channel_count() {
                    let mut v: Vec<Option<u8>> = Vec::with_capacity(ptn_len as usize);
                    notes.push(v);
                    let mut v: Vec<Option<u8>> = Vec::with_capacity(ptn_len as usize);
                    instruments.push(v);
                    let mut v: Vec<Option<u8>> = Vec::with_capacity(ptn_len as usize);
                    volumes.push(v);
                    let mut v: Vec<Option<u8>> = Vec::with_capacity(ptn_len as usize);
                    fx_commands.push(v);
                    let mut v: Vec<Option<u8>> = Vec::with_capacity(ptn_len as usize);
                    fx_params.push(v);
                }

                for _ in 0..ptn_len {
                    for chan in 0..xm.channel_count() {
                        let ctrl = xm.xmdata[file_offset];
                        
                        if ctrl & 0x80 != 0 {
                            file_offset += 1;
                            if ctrl & 1 != 0 {
                                notes[chan as usize].push(Some(xm.xmdata[file_offset]));
                                file_offset += 1;
                            }
                            else {
                                notes[chan as usize].push(None);
                            }
                            if ctrl & 2 != 0 {
                                instruments[chan as usize].push(Some(xm.xmdata[file_offset]));
                                file_offset += 1;
                            }
                            else {
                                instruments[chan as usize].push(None);
                            }
                            if ctrl & 4 != 0 {
                                volumes[chan as usize].push(Some(xm.xmdata[file_offset]));
                                file_offset += 1;
                            }
                            else {
                                volumes[chan as usize].push(None);
                            }
                            if ctrl & 8 != 0 {
                                fx_commands[chan as usize].push(Some(xm.xmdata[file_offset]));
                                file_offset += 1;
                            }
                            else {
                                fx_commands[chan as usize].push(None);
                            }
                            if ctrl & 0x10 != 0 {
                                fx_params[chan as usize].push(Some(xm.xmdata[file_offset]));
                                file_offset += 1;
                            }
                            else {
                                fx_params[chan as usize].push(None);
                            }
                        }
                        else {
                            notes[chan as usize].push(Some(xm.xmdata[file_offset]));
                            instruments[chan as usize].push(Some(xm.xmdata[file_offset + 1]));
                            volumes[chan as usize].push(Some(xm.xmdata[file_offset + 2]));
                            fx_commands[chan as usize].push(Some(xm.xmdata[file_offset + 3]));
                            fx_params[chan as usize].push(Some(xm.xmdata[file_offset + 4]));
                            file_offset += 5;
                        }
                    } 
                }

                let ptn = XMPattern {
                    notes: notes,
                    instruments: instruments,
                    volumes: volumes,
                    fx_commands: fx_commands,
                    fx_params: fx_params,
                };

                xm.patterns.push(ptn);

                if next_offset != file_offset {
                    return Err(XMParseError::new(&format!("{}: Pattern data corrupt.", filepath.display())));
                }
            }


            // parse instruments
            for _ in 0..xm.instrument_count() {
                let instr_offset = file_offset;
                let sample_count = xm.xmdata[file_offset + 27];
                // file_offset += instrument_header_size
                file_offset += xm.read_usize(file_offset);
                
                if sample_count > 0 {
                    let mut smp_header_offsets: Vec<usize> = Vec::with_capacity(sample_count as usize);
                    let mut smp_data_lengths: Vec<usize> = Vec::with_capacity(sample_count as usize);
                    
                    for _ in 0..sample_count {
                        smp_header_offsets.push(file_offset);
                        smp_data_lengths.push(xm.read_usize(file_offset));
                        file_offset += 40;
                    }

                    let mut smp_data_offsets: Vec<usize> = Vec::with_capacity(sample_count as usize);
                    for i in 0..sample_count {
                        smp_data_offsets.push(file_offset);
                        file_offset += smp_data_lengths[i as usize];
                    }

                    let instr = XMInstrument {
                        main_offset: instr_offset,
                        sample_header_offsets: Some(smp_header_offsets),
                        sample_data_offsets: Some(smp_data_offsets),
                    };

                    xm.instruments.push(instr);
                    //could do an integrity check of file_offset against sample_header_size here
                }
                else {
                    let instr = XMInstrument {
                        main_offset: instr_offset,
                        sample_header_offsets: None,
                        sample_data_offsets: None,
                    };

                    xm.instruments.push(instr);
                    file_offset += 27;
                }
            }

            Ok(xm)
        }

        /// Returns true if the Amiga frequency table is used, or false if the linear frequency table is used.
        pub fn amiga_ft(&self) -> bool {
            if self.xmdata[XM_FREQ_TABLE_TYPE] == 0 {
                return true;
            }
            else {
                return false;
            }
        }

        /// Returns the default BPM value.
        pub fn bpm(&self) -> u8 {
            self.xmdata[XM_DEFAULT_BPM]
        }

        /// Returns the number of channels used in the module.
        pub fn channel_count(&self) -> u8 {
            self.xmdata[XM_CHANNEL_COUNT]
        }

        /// Returns the number of instruments used in the module.
        pub fn instrument_count(&self) -> u8 {
            self.xmdata[XM_INSTRUMENT_COUNT]
        }

        /// Returns the sequence (song) length.
        pub fn len(&self) -> u16 {
            self.read_u16(XM_SEQUENCE_LEN)
        }

        // or should we perhaps return a &str?
        /// Returns the module name.
        pub fn name(&self) -> String {
            self.read_string(XM_MODULE_NAME, 20)
        }

        /// Returns the number of patterns used in the module.
        pub fn pattern_count(&self) -> u8 {
            self.xmdata[XM_PATTERN_COUNT]
        }

        /// Returns the sequence loop point (restart position)
        pub fn restart_pos(&self) -> u16 {
            self.read_u16(XM_RESTART_POS)
        }

        /// Returns the sequence (pattern order list)
        pub fn sequence(&self) -> Vec<u8> {
            self.xmdata[XM_SEQUENCE_BEGIN..(XM_SEQUENCE_BEGIN + self.len() as usize)].to_vec()
        }

        /// Returns default tempo value.
        pub fn tempo(&self) -> u8 {
            self.xmdata[XM_DEFAULT_TEMPO]
        }

        /// Returns the tracker name.
        pub fn tracker_name(&self) -> String {
            self.read_string(XM_TRACKER_NAME, 20)
        }

        /// Returns true if the given pattern is used in the sequence, false otherwise.
        pub fn pattern_used(&self, ptn: u8) -> bool {
            for it in &self.sequence() { 
                if ptn == *it { return true; }
            }

            false
        }

        fn read_u16(&self, offset: usize) -> u16 {
            self.xmdata[offset] as u16 + ((self.xmdata[offset + 1] as u16) << 8)
        }

        fn read_usize(&self, offset: usize) -> usize {
            self.xmdata[offset] as usize + ((self.xmdata[offset + 1] as usize) << 8)
                + ((self.xmdata[offset + 2] as usize) << 0x10) + ((self.xmdata[offset + 3] as usize) << 0x18)
        }

        fn read_string(&self, offset: usize, len: usize) -> String {
            let mut buf: Vec<u8> = Vec::with_capacity(len);
            let mut pos = offset;

            while pos <= offset + len && self.xmdata[pos] != 0 {
                buf.push(self.xmdata[pos]);
                pos += 1;
            }

            String::from_utf8_lossy(&buf).into_owned().trim_right().to_string()
        }

        fn verify_filetype(&self) -> Result<(), &'static str> {

            if self.xmdata.len() < 0x50 {
                return Err("Corrupted or invalid XM file.");
            }

            if &self.xmdata[..17] != "Extended Module: ".as_bytes() {
                return Err("Not an XM file.");
            }

            if self.xmdata[XM_VERSION_MINOR] != 4 || self.xmdata[XM_VERSION_MAJOR] != 1 {
                return Err("Not a version 1.04 XM file.")
            }

            Ok(())
        }
    }


    #[allow(dead_code, unused_variables)]
    #[derive(Default)]
    struct XMPattern {
        // triggers: Vec<bool>,
        notes: Vec<Vec<Option<u8>>>,
        instruments: Vec<Vec<Option<u8>>>,
        volumes: Vec<Vec<Option<u8>>>,
        fx_commands: Vec<Vec<Option<u8>>>,
        fx_params: Vec<Vec<Option<u8>>>,
    }


    #[derive(Default)]
    struct XMInstrument {
        // data: &Vec<u8>,
        main_offset: usize,
        sample_header_offsets: Option<Vec<usize>>,
        sample_data_offsets: Option<Vec<usize>>,
    }


    // #[derive(Default)]
    // struct XMSample {
    //     name: String,
    // }


    #[derive(Default, Debug)]
    pub struct XMParseError {
        why: String,
    }

    impl XMParseError {
        fn new(reason: &str) -> XMParseError {
            XMParseError{why: reason.to_string()}
        }
    }

    impl fmt::Display for XMParseError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.why)
        }
    }

    impl Error for XMParseError {
        fn description(&self) -> &str {
            &self.why
        }

        // fn cause(&self) -> Option<&Error> {
        //     // Generic error, underlying cause isn't tracked.
        //     None
        // }
    }
}

#[cfg(test)]
#[test]
fn it_works() {
    use std::path::Path;
    use std::error::Error;
    use xmkit;

    let xm = match xmkit::XModule::parse(&Path::new("test.xm")) {
        Err(e) => panic!("{}", e.description()),
        Ok(xm) => xm,
    };

    println!("Module name: {}", xm.name());
    println!("Made with: {}", xm.tracker_name());
    println!("Channels: {}", xm.channel_count());
    println!("Patterns: {}", xm.pattern_count());
    println!("Instruments: {}", xm.instrument_count());
    println!("Sequence length: {}", xm.len());
    println!("Restart position: {}", xm.restart_pos());
    println!("Using Amiga frequency table: {}", xm.amiga_ft());
    println!("BPM: {}", xm.bpm());
    println!("Tempo: {}", xm.tempo());
    println!("Sequence:");
    let mut pos = 0;
    for it in &xm.sequence() {
        // should be able to use {:02#x} as format!, but it's broken
        println!("0x{:02x}:\t0x{:02x}", pos, it);
        pos = pos + 1;
    }
    println!("Pattern 0 is used: {}", xm.pattern_used(0));
}
