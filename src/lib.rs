/// a great module it is
pub mod xmkit {
    use std::error::Error;
    use std::fmt;
    use std::fs;
    use std::io::prelude::*;
    use std::path::Path;
    use std::str;

    // #[derive(Default, Debug)]
    #[derive(Default)]
    pub struct XModule {
        xmdata: Vec<u8>,
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

            Ok(xm)
        }

        /// Returns the number of channels used in the module.
        pub fn channel_count(&self) -> u8 {
            self.xmdata[0x44]
        }

        // or should we perhaps return a &str?
        /// Returns the module name.
        pub fn name(&self) -> String {
            let mut buf: Vec<u8> = Vec::new();
            let mut i = 0x11;

            while i <= 0x25 && self.xmdata[i] != 0 {
                buf.push(self.xmdata[i]);
                i = i + 1;
            }

            String::from_utf8_lossy(&buf).into_owned()
        }

        /// Returns the tracker name.
        pub fn tracker_name(&self) -> String {
            let mut buf: Vec<u8> = Vec::new();

            for i in 0x26..0x3a {
                buf.push(self.xmdata[i]);
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

            if self.xmdata[0x3a] != 4 || self.xmdata[0x3b] != 1 {
                return Err("Not a version 1.04 XM file.")
            }

            Ok(())
        }
    }

    #[derive(Default, Debug)]
    pub struct XMParseError {
        msg: String,
    }

    impl XMParseError {
        fn new(_msg: &str) -> XMParseError {
            XMParseError{msg: _msg.to_string()}
        }
    }

    impl fmt::Display for XMParseError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.msg)
        }
    }

    impl Error for XMParseError {
        fn description(&self) -> &str {
            &self.msg
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

    println!("The module is called {}", xm.name());
    println!("It was made with {}", xm.tracker_name());
    println!("The number of channels is {}", xm.channel_count());

}
