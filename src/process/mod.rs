mod errors;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

pub use errors::Error;

pub struct Process<P, Q>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    binary_path: P,
    working_directory: Q,
    envs: HashMap<String, String>,
    timeout: Duration,
}

impl<P, Q> Process<P, Q>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    pub fn new(binary_path: P, working_directory: Q, envs: HashMap<String, String>, timeout: Duration) -> Self {
        Self {
            binary_path,
            working_directory,
            envs,
            timeout,
        }
    }

    pub fn spawn<I, S>(&self, args: I) -> Result<ProcessContext, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(self.binary_path.as_ref());
        let command = command
            .current_dir(self.working_directory.as_ref())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(args)
            .envs(&self.envs);

        let context = ProcessContext::new(command, self.timeout)?;

        Ok(context)
    }
}

pub struct ProcessContext {
    child: Child,
    start: Instant,
    timeout: Duration,

    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
    pub exit_code: Option<i32>,
    #[cfg(unix)]
    pub signal_code: Option<i32>,
}

impl ProcessContext {
    pub fn new(command: &mut Command, timeout: Duration) -> Result<Self, Error> {
        let start = Instant::now();

        Ok(Self {
            child: command.spawn()?,
            start,
            timeout,
            stdout: Vec::new(),
            stderr: Vec::new(),
            exit_code: None,
            #[cfg(unix)]
            signal_code: None,
        })
    }

    pub fn wait<'a, P, Q>(mut self, mut stdout: P, mut stderr: Q) -> Result<Self, Error>
    where
        P: 'a + FnMut(Option<String>),
        Q: 'a + FnMut(Option<String>),
    {
        let (stdout_tx, stdout_rx) = channel();
        let stdout_processor = StreamProcessor::new(self.child.stdout.take(), stdout_tx);

        let stdout_reader = std::thread::spawn(|| {
            stdout_processor.stream();
        });

        let (stderr_tx, stderr_rx) = channel();
        let stderr_processor = StreamProcessor::new(self.child.stderr.take(), stderr_tx);

        let stderr_reader = std::thread::spawn(|| {
            stderr_processor.stream();
        });

        loop {
            match self.child.try_wait() {
                Err(_) => {
                    let _ = self.child.kill().map(|_| self.child.wait());
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();

                    return Err(Error::TimeoutError);
                }
                Ok(Some(status)) => {
                    self.exit_code = status.code();

                    if cfg!(unix) {
                        self.signal_code = status.signal();
                    }

                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Ok(self);
                }
                Ok(None) => {
                    if self.start.elapsed().as_secs() < self.timeout.as_secs() {
                        std::thread::sleep(std::time::Duration::from_millis(20));

                        while let Ok(line) = stdout_rx.try_recv() {
                            if let Ok(line) = line {
                                stdout(Some(line.clone()));
                                self.stdout.push(line);
                            } else {
                                stdout(None);
                                self.stdout.push(String::from("<error retrieving stream content>"));
                            }
                        }

                        while let Ok(line) = stderr_rx.try_recv() {
                            if let Ok(line) = line {
                                stderr(Some(line.clone()));
                                self.stderr.push(line);
                            } else {
                                stderr(None);
                                self.stderr.push(String::from("<error retrieving stream content>"));
                            }
                        }

                        continue;
                    }

                    let _ = self.child.kill().map(|_| self.child.wait());
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(Error::TimeoutError);
                }
            };
        }
    }
}

pub struct StreamProcessor<T>
where
    T: Read,
{
    source: Option<T>,
    sender: Sender<Result<String, Error>>,
}

impl<T> StreamProcessor<T>
where
    T: Read,
{
    pub fn new(source: Option<T>, sender: Sender<Result<String, Error>>) -> Self {
        Self { source, sender }
    }

    fn stream(self) {
        if let Some(source) = self.source {
            for line in BufReader::new(source).lines().enumerate() {
                let (_, line) = line;
                let _ = self.sender.send(line.map_err(|e| Error::IOError(e.to_string())));
            }
        }
    }
}
