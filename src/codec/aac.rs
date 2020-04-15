use std::process::Stdio;

use derive_more::From;
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Command, Child, ChildStdin, ChildStdout, ChildStderr};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::codec::AudioStream;
use crate::engine::SAMPLE_RATE;

pub struct Aac {
    ffmpeg: Child,
    output: ChildStdout,
}

#[derive(From, Debug)]
pub enum AacError {
    #[from(ignore)]
    SpawnCodecProcess(io::Error),
}

impl Aac {
    pub async fn new(input: impl AsyncRead + Send + 'static) -> Result<Aac, AacError> {
        let mut ffmpeg = Command::new("ffmpeg")
            .args(&["-f", "aac", "-i", "-"]) // read aac from stdin
            .arg("-ar")
            .arg(&SAMPLE_RATE.to_string()) // resample to engine sample rate
            .args(&["-f", "f32le", "-"])  // write pcm32le to stdout
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(AacError::SpawnCodecProcess)?;

        // spawn task to send input data to ffmpeg
        tokio::spawn({
            let stdin = ffmpeg.stdin.take().unwrap();
            ffmpeg_input_task(stdin, input)
        });

        // stderr message reader
        let (messages_tx, mut messages_rx) = mpsc::channel(1);
        tokio::spawn({
            let stderr = ffmpeg.stderr.take().unwrap();
            ffmpeg_log_task(stderr, messages_tx)
        });

        while let Some(msg) = messages_rx.recv().await {
            println!("ffmpeg: {:?}", msg);
        }

        let mut output = ffmpeg.stdout.take().unwrap();

        let mut buff = [0u8; 4096];
        loop {
            let result = output.read(&mut buff).await;
            println!("read -> {:?}", result);
            if result.ok() == Some(0) {
                break;
            }
        }

        Ok(Aac { ffmpeg, output })
    }
}

async fn ffmpeg_input_task(mut ffmpeg: ChildStdin, mut input: impl AsyncRead) {
    let mut buff = [0u8; 4096];
    futures::pin_mut!(input);

    let mut f = std::fs::File::create("tmp.aac").unwrap();

    loop {
        match input.read(&mut buff).await {
            Ok(0) | Err(_) => break,
            Ok(bytes) => {
                use std::io::Write;
                f.write_all(&buff[0..bytes]);

                match ffmpeg.write_all(&buff[0..bytes]).await {
                    Ok(()) => {}
                    Err(_) => break,
                }

                match ffmpeg.flush().await {
                    Ok(()) => {}
                    Err(_) => break,
                }
            }
        }
    }

    println!("exiting ffmpeg_input_task!");
}

#[derive(Debug)]
enum MessageReadError {
    Io(io::Error),
    Utf8,
}

async fn ffmpeg_log_task(mut ffmpeg: ChildStderr, mut messages: Sender<Result<String, MessageReadError>>) {
    let mut buff = [0u8; 128];
    let mut line_buff = vec![];

    'main_loop: loop {
        match ffmpeg.read(&mut buff).await {
            Ok(0) => {
                let line = String::from_utf8(line_buff)
                    .map_err(|_| MessageReadError::Utf8);

                // we're exiting this task anyway, nothing to do about any error here:
                let _: Result<_, _> = messages.send(line).await;
                println!("*** ffmpeg read 0 bytes");
                break;
            }
            Err(e) => {
                println!("*** couldny read frae ffmpeg: {:?}", e);
                let _: Result<_, _> = messages.send(Err(MessageReadError::Io(e))).await;
                break;
            }
            Ok(bytes) => {
                let mut start_idx = 0;

                while start_idx < bytes {
                    let end_idx = bytes - start_idx;

                    let pos_after_separator = buff[start_idx..bytes].iter()
                        .position(|sep| {
                            match sep {
                                // some messages from ffmpeg are lines (terminated
                                // by \n), and some are progress updates (delimited
                                // by \r). we need to split on both
                                b'\r' | b'\n' => true,
                                _ => false,
                            }
                        })
                        .map(|pos| start_idx + pos + 1);

                    match pos_after_separator {
                        Some(pos_after_separator) => {
                            line_buff.extend(&buff[start_idx..pos_after_separator]);

                            let line = String::from_utf8(line_buff)
                                .map_err(|_| MessageReadError::Utf8);

                            line_buff = Vec::new();

                            if let Err(e) = messages.send(line).await {
                                println!("*** couldny send tae messages: {:?}", e);
                                break 'main_loop;
                            }

                            start_idx = pos_after_separator;
                        }
                        None => {
                            line_buff.extend(&buff[start_idx..bytes]);
                            break;
                        }
                    }
                }
            }
        }
    }
}
