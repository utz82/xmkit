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
    pub const XM_ENVELOPE_ON: u8 = 0x1;
    pub const XM_ENVELOPE_SUSTAIN: u8 = 0x2;
    pub const XM_ENVELOPE_LOOP: u8 = 0x4;
    pub const XM_SAMPLE_LOOP_NONE: u8 = 0x1;
    pub const XM_SAMPLE_LOOP_FORWARD: u8 = 0x2;
    pub const XM_SAMPLE_LOOP_PINGPONG: u8 = 0x4;
    pub const XM_SAMPLE_16BIT: u8 = 0x10;


    #[derive(Default)]
    pub struct XModule {
        header: Vec<u8>,
        pub patterns: Vec<XMPattern>,
        pub instruments: Vec<XMInstrument>,
    }

    impl XModule {
       
        /// Opens and parses an eXtended Module (XM) file, and constructs an XModule instance from it if the XM file is valid.
        pub fn parse_file(filepath: &Path) -> Result<XModule, XMParseError> {
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

            let mut data: Vec<u8> = Vec::with_capacity(metadata.len() as usize);
            fs::File::read_to_end(&mut xmfile, &mut data).unwrap();

            XModule::parse(data)
        }

        /// Parses eXtended Module data, and constructs an XModule instance from it if the data is valid. 
        pub fn parse(data: Vec<u8>) -> Result<XModule, XMParseError> {

            XModule::verify_filetype(&data)?;

            let mut xm: XModule = Default::default();

            // calculate beginning of pattern data; stored header size 
            // does not include bytes up to XM_HEADER_SIZE offset (0x3c)
            let mut file_offset: usize = XM_HEADER_SIZE + XModule::read_usize(&data, XM_HEADER_SIZE);
            xm.header = data[..file_offset].to_vec();
            let channel_count = xm.channel_count();

            // parse pattern data
            for _ in 0..xm.pattern_count() {
                let ptn_size = XModule::read_usize(&data, file_offset) + (XModule::read_u16(&data, file_offset + 7) as usize);

                xm.patterns.push(XMPattern::parse(data[file_offset..(file_offset + ptn_size)].to_vec(), channel_count)?);
                file_offset += ptn_size;
            }

            // parse instruments
            for _ in 0..xm.instrument_count() {
                let instr_offset = file_offset;
                let sample_count = data[file_offset + 27];
                file_offset += XModule::read_usize(&data, file_offset);

                if sample_count == 0 {
                    file_offset += 29;
                }
                else {
                    let mut data_length: usize = 0;
                    for _ in 0..sample_count {
                        data_length += XModule::read_usize(&data, file_offset);
                        file_offset += 40;
                    }
                    file_offset += data_length;
                }

                match XMInstrument::parse(data[instr_offset..file_offset].to_vec()) {
                    Err(e) => return Err(e),
                    Ok(instr) => xm.instruments.push(instr),
                }
            }

            Ok(xm)
        }

        /// Returns true if the Amiga frequency table is used, or false if the linear frequency table is used.
        pub fn amiga_ft(&self) -> bool {
            if self.header[XM_FREQ_TABLE_TYPE] == 0 {
                return true;
            }
            else {
                return false;
            }
        }

        /// Returns the default BPM value.
        pub fn bpm(&self) -> u8 {
            self.header[XM_DEFAULT_BPM]
        }

        /// Returns the number of channels used in the module.
        pub fn channel_count(&self) -> u8 {
            self.header[XM_CHANNEL_COUNT]
        }

        /// Returns the number of instruments used in the module.
        pub fn instrument_count(&self) -> u8 {
            self.header[XM_INSTRUMENT_COUNT]
        }

        /// Returns the sequence (song) length.
        pub fn len(&self) -> u16 {
            // self.read_u16(XM_SEQUENCE_LEN)
            XModule::read_u16(&self.header, XM_SEQUENCE_LEN)
        }

        // or should we perhaps return a &str?
        /// Returns the module name.
        pub fn name(&self) -> String {
            XModule::read_string(&self.header, XM_MODULE_NAME, 20)
        }

        /// Returns the number of patterns used in the module.
        pub fn pattern_count(&self) -> u8 {
            self.header[XM_PATTERN_COUNT]
        }

        /// Returns the sequence loop point (restart position)
        pub fn restart_pos(&self) -> u16 {
            XModule::read_u16(&self.header, XM_RESTART_POS)
        }

        /// Returns the sequence (pattern order list)
        pub fn sequence(&self) -> Vec<u8> {
            self.header[XM_SEQUENCE_BEGIN..(XM_SEQUENCE_BEGIN + self.len() as usize)].to_vec()
        }

        /// Returns default tempo value.
        pub fn tempo(&self) -> u8 {
            self.header[XM_DEFAULT_TEMPO]
        }

        /// Returns the tracker name.
        pub fn tracker_name(&self) -> String {
            XModule::read_string(&self.header, XM_TRACKER_NAME, 20)
        }

        /// Returns true if the given pattern is used in the sequence, false otherwise.
        pub fn pattern_used(&self, ptn: u8) -> bool {
            for it in &self.sequence() { 
                if ptn == *it { return true; }
            }

            false
        }

        fn read_u16(data: &Vec<u8>, offset: usize) -> u16 {
            data[offset] as u16 + ((data[offset + 1] as u16) << 8)
        }

        fn read_usize(data: &Vec<u8>, offset: usize) -> usize {
            data[offset] as usize + ((data[offset + 1] as usize) << 8)
                + ((data[offset + 2] as usize) << 0x10) + ((data[offset + 3] as usize) << 0x18)
        }

        // TODO should check if there's enough data in buffer, and throw an XMParseError if not
        fn read_string(data: &Vec<u8>, offset: usize, len: usize) -> String {
            let mut buf: Vec<u8> = Vec::with_capacity(len);
            let mut pos = offset;

            while pos <= offset + len && data[pos] != 0 {
                buf.push(data[pos]);
                pos += 1;
            }

            String::from_utf8_lossy(&buf).into_owned().trim_right().to_string()
        }

        fn verify_filetype(data: &Vec<u8>) -> Result<(), XMParseError> {

            if data.len() < 60 || data.len() < 60 + XModule::read_usize(&data, XM_HEADER_SIZE) {
                return Err(XMParseError::new("Corrupted or invalid XM data."));
            }

            if data[..17].to_vec() != "Extended Module: ".as_bytes() {
                return Err(XMParseError::new("Not an eXtended Module."));
            }

            if data[XM_VERSION_MINOR] != 4 || data[XM_VERSION_MAJOR] != 1 {
                return Err(XMParseError::new("XM data not from version 1.04 XM standard."));
            }

            Ok(())
        }
    }


    #[allow(dead_code, unused_variables)]
    #[derive(Default)]
    pub struct XMPattern {
        header: Vec<u8>,
        channel_count: u8,
        notes: Vec<Vec<Option<u8>>>,
        instruments: Vec<Vec<Option<u8>>>,
        volumes: Vec<Vec<Option<u8>>>,
        fx_commands: Vec<Vec<Option<u8>>>,
        fx_params: Vec<Vec<Option<u8>>>,
    }

    impl XMPattern {

        /// Parses eXtended Module pattern data, and constructs an XMPattern instance from it if the data is valid.
        pub fn parse(data: Vec<u8>, channel_count: u8) -> Result<XMPattern, XMParseError> {

            if data.len() < 9 || data.len() != XModule::read_usize(&data, 0) + (XModule::read_u16(&data, 7) as usize) {
                return Err(XMParseError::new("XM Pattern data corrupt or incomplete."))
            }

            let mut ptn: XMPattern = Default::default();
            ptn.channel_count = channel_count;
            ptn.header = data[0..XModule::read_usize(&data, 0)].to_vec();

            let mut file_offset = 0;
            let ptn_len = data[5];
            file_offset += XModule::read_usize(&data, file_offset);

            for _ in 0..channel_count {
                let v: Vec<Option<u8>> = Vec::with_capacity(ptn_len as usize);
                ptn.notes.push(v);
                let v: Vec<Option<u8>> = Vec::with_capacity(ptn_len as usize);
                ptn.instruments.push(v);
                let v: Vec<Option<u8>> = Vec::with_capacity(ptn_len as usize);
                ptn.volumes.push(v);
                let v: Vec<Option<u8>> = Vec::with_capacity(ptn_len as usize);
                ptn.fx_commands.push(v);
                let v: Vec<Option<u8>> = Vec::with_capacity(ptn_len as usize);
                ptn.fx_params.push(v);
            }

            for _ in 0..ptn_len {
                for chan in 0..channel_count {
                    let ctrl = data[file_offset];
                    
                    if ctrl & 0x80 != 0 {
                        file_offset += 1;
                        if ctrl & 1 != 0 {
                            ptn.notes[chan as usize].push(Some(data[file_offset]));
                            file_offset += 1;
                        }
                        else {
                            ptn.notes[chan as usize].push(None);
                        }
                        if ctrl & 2 != 0 {
                            ptn.instruments[chan as usize].push(Some(data[file_offset]));
                            file_offset += 1;
                        }
                        else {
                            ptn.instruments[chan as usize].push(None);
                        }
                        if ctrl & 4 != 0 {
                            ptn.volumes[chan as usize].push(Some(data[file_offset]));
                            file_offset += 1;
                        }
                        else {
                            ptn.volumes[chan as usize].push(None);
                        }
                        if ctrl & 8 != 0 {
                            ptn.fx_commands[chan as usize].push(Some(data[file_offset]));
                            file_offset += 1;
                        }
                        else {
                            ptn.fx_commands[chan as usize].push(None);
                        }
                        if ctrl & 0x10 != 0 {
                            ptn.fx_params[chan as usize].push(Some(data[file_offset]));
                            file_offset += 1;
                        }
                        else {
                            ptn.fx_params[chan as usize].push(None);
                        }
                    }
                    else {
                        ptn.notes[chan as usize].push(Some(data[file_offset]));
                        ptn.instruments[chan as usize].push(Some(data[file_offset + 1]));
                        ptn.volumes[chan as usize].push(Some(data[file_offset + 2]));
                        ptn.fx_commands[chan as usize].push(Some(data[file_offset + 3]));
                        ptn.fx_params[chan as usize].push(Some(data[file_offset + 4]));
                        file_offset += 5;
                    }
                } 
            }

            Ok(ptn)
        }
    }


    #[derive(Default)]
    pub struct XMTrack {
        notes: Vec<Option<u8>>,
        instruments: Vec<Option<u8>>,
        volumes: Vec<Option<u8>>,
        fx_commands: Vec<Option<u8>>,
        fx_params: Vec<Option<u8>>,
    }


    #[derive(Default)]
    pub struct XMInstrument {
        header: Vec<u8>,
        pub samples: Vec<XMSample>,
    }

    impl XMInstrument {

        /// Parses eXtended Module instrument data, and constructs an XMInstrument instance from it if the data is valid.
        pub fn parse(data: Vec<u8>) -> Result<XMInstrument, XMParseError> {
            let mut instr: XMInstrument = Default::default();
            let sample_count = data[27] as usize;

            if sample_count > 0 {
                instr.header = data[..XModule::read_usize(&data, 0)].to_vec();
                let mut instr_samples = Vec::with_capacity(sample_count);
                let mut header_offset: usize = instr.header.len();
                let mut data_offset: usize = sample_count * 40;
                
                for _ in 0..sample_count {
                    instr_samples.push(XMSample{
                        header: data[header_offset..(header_offset+40)].to_vec(),
                        data: data[data_offset..XModule::read_usize(&data, header_offset)].to_vec(),
                    });

                    header_offset += 40;
                    data_offset += XModule::read_usize(&data, header_offset);
                }
                instr.samples = instr_samples;
            }
            else {
                instr.header = data[..29].to_vec();
            }

            Ok(instr)
        }

        /// Returns the name of the instrument, or an empty string if the instrument is unnamed.
        pub fn name(&self) -> String {
            XModule::read_string(&self.header, 4, 22)
        }

        /// Returns the points of the instrument's panning envelope, or None of the instrument has no samples,
        /// or if there are no points in the envelope.
        pub fn panning_envelope(&self) -> Option<Vec<u8>> {
            if self.sample_count() == 0 || self.header[226] == 0 { None }
            else {
                Some(self.header[177..(177 + (self.header[226] as usize))].to_vec())
            }
        }

        /// Returns the volume loop start point; or None if the instrument has no samples, 
        /// the volume envelope has no points, or volume envelope looping is inactive.
        pub fn panning_loop_start(&self) -> Option<u8> {
            if self.sample_count() == 0 || self.header[225] == 0 || self.header[233] & XM_ENVELOPE_LOOP == 0 { None }
            else {
                Some(self.header[231])
            }
        }

        /// Returns the volume loop end point; or None if the instrument has no samples, 
        /// the volume envelope has no points, or volume envelope looping is inactive.
        pub fn panning_loop_end(&self) -> Option<u8> {
            if self.sample_count() == 0 || self.header[225] == 0 || self.header[233] & XM_ENVELOPE_LOOP == 0 { None }
            else {
                Some(self.header[232])
            }
        }

        /// Returns the volume loop sustain point; or None if the instrument has no samples, 
        /// or the volume envelope has no points.
        pub fn panning_sustain(&self) -> Option<u8> {
            if self.sample_count() == 0 || self.header[225] == 0 { None }
            else {
                Some(self.header[230])
            }
        }

        /// Return the panning envelope type, or None of the instrument has no samples.
        /// If Some result is returned, it will be a bitmask that can be checked against
        /// the XM_ENVELOPE_ON, XM_ENVELOPE_SUSTAIN, and XM_ENVELOPE_LOOP flags.
        pub fn panning_type(&self) -> Option<u8> {
            if self.sample_count() == 0 { None }
            else {
                Some(self.header[234])
            }
        }

        /// Returns the number of samples contained by the instrument.
        pub fn sample_count(&self) -> u8 {
            self.header[27]
        }

        /// Returns the sample number for each note, or None if the instrument does not contain any samples.
        /// You might nevertheless want to check the results of sample_count() before calling this function,
        /// since the output will likely be useless if there is only one sample in the instrument.
        pub fn sample_numbers(&self) -> Option<Vec<u8>> {
            if self.sample_count() == 0 { None }
            else {
                Some(self.header[33..129].to_vec())
            }
        }

        /// Returns the vibrato depth setting, or None of the instrument has no samples.
        pub fn vibrato_depth(&self) -> Option<u8> {
            if self.sample_count() == 0 { None }
            else {
                Some(self.header[237])
            }
        }

        /// Returns the vibrato rate setting, or None of the instrument has no samples.
        pub fn vibrato_rate(&self) -> Option<u8> {
            if self.sample_count() == 0 { None }
            else {
                Some(self.header[238])
            }
        }

        /// Returns the vibrato sweep setting, or None of the instrument has no samples.
        pub fn vibrato_sweep(&self) -> Option<u8> {
            if self.sample_count() == 0 { None }
            else {
                Some(self.header[236])
            }
        }

        /// Returns the vibrato type setting, or None of the instrument has no samples.
        pub fn vibrato_type(&self) -> Option<u8> {
            if self.sample_count() == 0 { None }
            else {
                Some(self.header[235])
            }
        }

        /// Returns the points of the instrument's volume envelope, or None of the instrument has no samples,
        /// or if there are no points in the envelope.
        pub fn volume_envelope(&self) -> Option<Vec<u8>> {
            if self.sample_count() == 0 || self.header[225] == 0 { None }
            else {
                Some(self.header[129..(129 + (self.header[225] as usize))].to_vec())
            }
        }
        
        /// Returns the volume fadeout setting, or None of the instrument has no samples.
        pub fn volume_fadeout(&self) -> Option<u16> {
            if self.sample_count() == 0 { None }
            else {
                Some(self.header[239] as u16 + ((self.header[240] as u16) << 8))
            }
        }

        /// Returns the volume loop start point; or None if the instrument has no samples, 
        /// the volume envelope has no points, or volume envelope looping is inactive.
        pub fn volume_loop_start(&self) -> Option<u8> {
            if self.sample_count() == 0 || self.header[225] == 0 || self.header[233] & XM_ENVELOPE_LOOP == 0 { None }
            else {
                Some(self.header[228])
            }
        }

        /// Returns the volume loop end point; or None if the instrument has no samples, 
        /// the volume envelope has no points, or volume envelope looping is inactive.
        pub fn volume_loop_end(&self) -> Option<u8> {
            if self.sample_count() == 0 || self.header[225] == 0 || self.header[233] & XM_ENVELOPE_LOOP == 0 { None }
            else {
                Some(self.header[229])
            }
        }

        /// Returns the volume loop sustain point; or None if the instrument has no samples, 
        /// or the volume envelope has no points.
        pub fn volume_sustain(&self) -> Option<u8> {
            if self.sample_count() == 0 || self.header[225] == 0 { None }
            else {
                Some(self.header[227])
            }
        }

        /// Return the volume envelope type, or None of the instrument has no samples.
        /// If Some result is returned, it will be a bitmask that can be checked against
        /// the XM_ENVELOPE_ON, XM_ENVELOPE_SUSTAIN, and XM_ENVELOPE_LOOP flags.
        pub fn volume_type(&self) -> Option<u8> {
            if self.sample_count() == 0 { None }
            else {
                Some(self.header[233])
            }
        }
    }


    #[derive(Default)]
    pub struct XMSample {
        header: Vec<u8>,
        data: Vec<u8>,
    }

    impl XMSample {
        /// Returns true if the sample data has 16-bit resolution, false if it has 8-bit resolution.
        pub fn is_16bit(&self) -> bool {
            if self.header[14] & 0x10 == 0 { false }
            else { true }
        }

        // pub fn data_8bit_signed(&self) -> Vec<i8> {

        // }

        // pub fn data_8bit_unsigned(&self) -> Vec<u8> {

        // }

        // pub fn data_16bit_signed(&self) -> Vec<i16> {

        // }

        // pub fn data_16bit_unsigned(&self) -> Vec<u16> {

        // }

        /// Returns the sample data in XM's native delta format.
        /// Use is_16bit() to check the data resolution.
        pub fn data_native(&self) -> Vec<u8> {
            self.data[..].to_vec()
        }

        /// Returns the finetune setting. The result will be a signed value between -16 and +15.
        pub fn finetune(&self) -> i8 {
            self.header[13] as i8
        }

        /// Returns the lenght of the raw sample data.
        pub fn len(&self) -> usize {
            XModule::read_usize(&self.header, 0)
        }

        /// Returns the loop length setting.
        pub fn loop_len(&self) -> usize {
            XModule::read_usize(&self.header, 8)
        }

        /// Returns the loop start setting.
        pub fn loop_start(&self) -> usize {
            XModule::read_usize(&self.header, 4)
        }

        /// Returns the loop type used by the sample.
        /// This will evaluate to one of XM_SAMPLE_LOOP_NONE, XM_SAMPLE_LOOP_FORWARD, or XM_SAMPLE_LOOP_PINGPONG.
        pub fn loop_type(&self) -> u8 {
            if self.header[14] & 1 != 0 { XM_SAMPLE_LOOP_NONE }
            else if self.header[14] & 2 != 0 { XM_SAMPLE_LOOP_FORWARD }
            else { XM_SAMPLE_LOOP_PINGPONG }
        }

        /// Returns the name of the sample.
        pub fn name(&self) -> String {
            XModule::read_string(&self.header, 18, 22)
        }

        /// Returns the panning setting.
        pub fn panning(&self) -> u8 {
            self.header[15]
        }

        /// Returns the relative note setting.
        pub fn relative_note(&self) -> i8 {
            self.header[16] as i8
        }

        /// Returns the volume setting.
        pub fn volume(&self) -> u8 {
            self.header[12]
        }
    }


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
fn test_all() {
    use std::path::Path;
    use std::error::Error;
    use xmkit;

    let xm = match xmkit::XModule::parse_file(&Path::new("test.xm")) {
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

    println!("Instruments:");

    for it in xm.instruments.iter() {
        println!("{}", it.name());

        if it.sample_count() > 0 {
            for smp in it.samples.iter() {
                println!("\t{}", smp.name());
            }
        }

        if it.sample_count() > 1 {
            println!("Sample numbers:");
        
            for sn in &it.sample_numbers().unwrap() {
                print!("{},", sn);
            }
        
            println!("");
        }
    }
}
