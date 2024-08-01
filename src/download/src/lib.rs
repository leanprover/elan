//! Easy file downloading

#[macro_use]
extern crate error_chain;
extern crate url;

use std::path::Path;
use url::Url;

mod errors;
pub use errors::*;

#[derive(Debug, Copy, Clone)]
pub enum Backend {
    Curl,
}

#[derive(Debug, Copy, Clone)]
pub enum Event<'a> {
    /// Received the Content-Length of the to-be downloaded data.
    DownloadContentLengthReceived(u64),
    /// Received some data.
    DownloadDataReceived(&'a [u8]),
}

fn download_with_backend(
    backend: Backend,
    url: &Url,
    callback: &dyn Fn(Event) -> Result<()>,
) -> Result<()> {
    match backend {
        Backend::Curl => curl::download(url, callback),
    }
}

pub fn download_to_path_with_backend(
    backend: Backend,
    url: &Url,
    path: &Path,
    callback: Option<&dyn Fn(Event) -> Result<()>>,
) -> Result<()> {
    use std::cell::RefCell;
    use std::fs::OpenOptions;
    use std::io::Write;

    || -> Result<()> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&path)
            .chain_err(|| "error creating file for download")?;

        let file = RefCell::new(file);

        download_with_backend(backend, url, &|event| {
            if let Event::DownloadDataReceived(data) = event {
                file.borrow_mut()
                    .write_all(data)
                    .chain_err(|| "unable to write download to disk")?;
            }
            match callback {
                Some(cb) => cb(event),
                None => Ok(()),
            }
        })?;

        file.borrow_mut()
            .sync_data()
            .chain_err(|| "unable to sync download to disk")?;

        Ok(())
    }()
    .map_err(|e| {
        // TODO is there any point clearing up here? What kind of errors will leave us with an unusable partial?
        e
    })
}

/// Download via libcurl; encrypt with the native (or OpenSSl) TLS
/// stack via libcurl
#[cfg(feature = "curl-backend")]
pub mod curl {

    extern crate curl;

    use self::curl::easy::Easy;
    use super::Event;
    use errors::*;
    use std::cell::RefCell;
    use std::str;
    use std::time::Duration;
    use url::Url;

    thread_local!(pub static EASY: RefCell<Easy> = RefCell::new(Easy::new()));

    pub fn download(url: &Url, callback: &dyn Fn(Event) -> Result<()>) -> Result<()> {
        // Fetch either a cached libcurl handle (which will preserve open
        // connections) or create a new one if it isn't listed.
        //
        // Once we've acquired it, reset the lifetime from 'static to our local
        // scope.
        EASY.with(|handle| {
            let mut handle = handle.borrow_mut();

            handle
                .url(&url.to_string())
                .chain_err(|| "failed to set url")?;
            handle
                .follow_location(true)
                .chain_err(|| "failed to set follow redirects")?;

            // Take at most 30s to connect
            handle
                .connect_timeout(Duration::new(30, 0))
                .chain_err(|| "failed to set connect timeout")?;

            {
                let cberr = RefCell::new(None);
                let mut transfer = handle.transfer();

                // Data callback for libcurl which is called with data that's
                // downloaded. We just feed it into our hasher and also write it out
                // to disk.
                transfer
                    .write_function(|data| match callback(Event::DownloadDataReceived(data)) {
                        Ok(()) => Ok(data.len()),
                        Err(e) => {
                            *cberr.borrow_mut() = Some(e);
                            Ok(0)
                        }
                    })
                    .chain_err(|| "failed to set write")?;

                // Listen for headers and parse out a `Content-Length` if it comes
                // so we know how much we're downloading.
                transfer
                    .header_function(|header| {
                        if let Ok(data) = str::from_utf8(header) {
                            let prefix = "Content-Length: ";
                            if data.starts_with(prefix) {
                                if let Ok(s) = data[prefix.len()..].trim().parse::<u64>() {
                                    let msg = Event::DownloadContentLengthReceived(s);
                                    match callback(msg) {
                                        Ok(()) => (),
                                        Err(e) => {
                                            *cberr.borrow_mut() = Some(e);
                                            return false;
                                        }
                                    }
                                }
                            }
                        }
                        true
                    })
                    .chain_err(|| "failed to set header")?;

                // If an error happens check to see if we had a filesystem error up
                // in `cberr`, but we always want to punt it up.
                transfer.perform().or_else(|e| {
                    // If the original error was generated by one of our
                    // callbacks, return it.
                    match cberr.borrow_mut().take() {
                        Some(cberr) => Err(cberr),
                        None => {
                            // Otherwise, return the error from curl
                            if e.is_file_couldnt_read_file() {
                                Err(e).chain_err(|| ErrorKind::FileNotFound)
                            } else {
                                Err(e).chain_err(|| "error during download")
                            }
                        }
                    }
                })?;
            }

            // If we didn't get a 20x or 0 ("OK" for files) then return an error
            let code = handle
                .response_code()
                .chain_err(|| "failed to get response code")?;
            match code {
                0 | 200..=299 => {}
                _ => {
                    return Err(ErrorKind::HttpStatus(code).into());
                }
            };

            Ok(())
        })
    }
}
