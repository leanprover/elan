use elan::Notification;
use elan_dist::Notification as In;
use elan_utils::tty;
use elan_utils::Notification as Un;
use std::collections::VecDeque;
use std::fmt;
use time::OffsetDateTime;

/// Keep track of this many past download amounts
const DOWNLOAD_TRACK_COUNT: usize = 5;

/// Tracks download progress and displays information about it to a terminal.
pub struct DownloadTracker {
    /// Content-Length of the to-be downloaded object.
    content_len: Option<u64>,
    /// Total data downloaded in bytes.
    total_downloaded: usize,
    /// Data downloaded this second.
    downloaded_this_sec: usize,
    /// Keeps track of amount of data downloaded every last few secs.
    /// Used for averaging the download speed.
    downloaded_last_few_secs: VecDeque<usize>,
    /// Time stamp of the last second
    last_sec: Option<f64>,
    /// How many seconds have elapsed since the download started
    seconds_elapsed: u32,
    /// The terminal we write the information to.
    term: Option<Box<term::StdoutTerminal>>,
    /// Whether we displayed progress for the download or not.
    ///
    /// If the download is quick enough, we don't have time to
    /// display the progress info.
    /// In that case, we do not want to do some cleanup stuff we normally do.
    ///
    /// If we have displayed progress, this is the number of characters we
    /// rendered, so we can erase it cleanly.
    displayed_charcount: Option<usize>,
}

impl DownloadTracker {
    /// Creates a new DownloadTracker.
    pub fn new() -> Self {
        DownloadTracker {
            content_len: None,
            total_downloaded: 0,
            downloaded_this_sec: 0,
            downloaded_last_few_secs: VecDeque::with_capacity(DOWNLOAD_TRACK_COUNT),
            seconds_elapsed: 0,
            last_sec: None,
            term: term::stdout(),
            displayed_charcount: None,
        }
    }

    pub fn handle_notification(&mut self, n: &Notification<'_>) -> bool {
        match *n {
            Notification::Install(In::Utils(Un::DownloadContentLengthReceived(content_len))) => {
                self.content_length_received(content_len);

                true
            }
            Notification::Install(In::Utils(Un::DownloadDataReceived(data))) => {
                if tty::stdout_isatty() && self.term.is_some() {
                    self.data_received(data.len());
                }
                true
            }
            Notification::Install(In::Utils(Un::DownloadFinished)) => {
                self.download_finished();
                true
            }
            _ => false,
        }
    }

    /// Notifies self that Content-Length information has been received.
    pub fn content_length_received(&mut self, content_len: u64) {
        self.content_len = Some(content_len);
    }
    /// Notifies self that data of size `len` has been received.
    pub fn data_received(&mut self, len: usize) {
        self.total_downloaded += len;
        self.downloaded_this_sec += len;

        let current_time: f64 =
            (OffsetDateTime::now_utc() - OffsetDateTime::UNIX_EPOCH).as_seconds_f64();

        match self.last_sec {
            None => self.last_sec = Some(current_time),
            Some(start) => {
                let elapsed = current_time - start;
                if elapsed >= 1.0 {
                    self.seconds_elapsed += 1;

                    self.display();
                    self.last_sec = Some(current_time);
                    if self.downloaded_last_few_secs.len() == DOWNLOAD_TRACK_COUNT {
                        self.downloaded_last_few_secs.pop_back();
                    }
                    self.downloaded_last_few_secs
                        .push_front(self.downloaded_this_sec);
                    self.downloaded_this_sec = 0;
                }
            }
        }
    }
    /// Notifies self that the download has finished.
    pub fn download_finished(&mut self) {
        if self.displayed_charcount.is_some() {
            // Display the finished state
            self.display();
            let _ = writeln!(self.term.as_mut().unwrap());
        }
        self.prepare_for_new_download();
    }
    /// Resets the state to be ready for a new download.
    fn prepare_for_new_download(&mut self) {
        self.content_len = None;
        self.total_downloaded = 0;
        self.downloaded_this_sec = 0;
        self.downloaded_last_few_secs.clear();
        self.seconds_elapsed = 0;
        self.last_sec = None;
        self.displayed_charcount = None;
    }
    /// Display the tracked download information to the terminal.
    fn display(&mut self) {
        let total_h = HumanReadable(self.total_downloaded as f64);
        let sum = self
            .downloaded_last_few_secs
            .iter()
            .fold(0., |a, &v| a + v as f64);
        let len = self.downloaded_last_few_secs.len();
        let speed = if len > 0 { sum / len as f64 } else { 0. };
        let speed_h = HumanReadable(speed);

        // First, move to the start of the current line and clear it.
        let _ = write!(self.term.as_mut().unwrap(), "\r");
        // We'd prefer to use delete_line() but on Windows it seems to
        // sometimes do unusual things
        // let _ = self.term.as_mut().unwrap().delete_line();
        // So instead we do:
        if let Some(n) = self.displayed_charcount {
            // This is not ideal as very narrow terminals might mess up,
            // but it is more likely to succeed until term's windows console
            // fixes whatever's up with delete_line().
            let _ = write!(self.term.as_mut().unwrap(), "{}", " ".repeat(n));
            let _ = self.term.as_mut().unwrap().flush();
            let _ = write!(self.term.as_mut().unwrap(), "\r");
        }

        let output: String = match self.content_len {
            Some(content_len) => {
                let content_len = content_len as f64;
                let percent = (self.total_downloaded as f64 / content_len) * 100.;
                let content_len_h = HumanReadable(content_len);
                let remaining = content_len - self.total_downloaded as f64;
                let eta_h = HumanReadable(remaining / speed);
                format!(
                    "{} / {} ({:3.0} %) {}/s ETA: {:#}",
                    total_h, content_len_h, percent, speed_h, eta_h
                )
            }
            None => {
                format!("Total: {} Speed: {}/s", total_h, speed_h)
            }
        };

        let _ = write!(self.term.as_mut().unwrap(), "{output}");
        // Since stdout is typically line-buffered and we don't print a newline, we manually flush.
        let _ = self.term.as_mut().unwrap().flush();
        self.displayed_charcount = Some(output.chars().count());
    }
}

/// Human readable representation of data size in bytes
struct HumanReadable(f64);

impl fmt::Display for HumanReadable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            // repurposing the alternate mode for ETA
            let sec = self.0;

            if sec.is_infinite() {
                write!(f, "Unknown")
            } else if sec > 1e3 {
                let sec = self.0 as u64;
                let min = sec / 60;
                let sec = sec % 60;

                write!(f, "{:3} min {:2} s", min, sec) // XYZ min PQ s
            } else {
                write!(f, "{:3.0} s", self.0) // XYZ s
            }
        } else {
            const KIB: f64 = 1024.0;
            const MIB: f64 = KIB * KIB;
            let size = self.0;

            if size >= MIB {
                write!(f, "{:5.1} MiB", size / MIB) // XYZ.P MiB
            } else if size >= KIB {
                write!(f, "{:5.1} KiB", size / KIB)
            } else {
                write!(f, "{:3.0} B", size)
            }
        }
    }
}
