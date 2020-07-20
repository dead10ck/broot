use {
    super::Message,
    crate::{
        errors::NetError,
        command::Sequence,
    },
    crossbeam::channel::Sender,
    std::{
        fs,
        io::BufReader,
        os::unix::net::UnixListener,
        thread,
    },
};

pub struct Server {
    path: String,
}

impl Server {
    pub fn new(name: &str, tx: Sender<Sequence>) -> Result<Self, NetError> {
        let path = super::socket_file_path(name);
        if fs::metadata(&path).is_ok() {
            return Err(NetError::SocketNotAvailable { path });
        }
        let listener = UnixListener::bind(&path)?;
        debug!("listening on {}", &path);

        // we use only one thread as we don't want to support long connections
        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let mut br = BufReader::new(stream);
                        if let Some(sequence) = match Message::read(&mut br) {
                            Ok(Message::Command(command)) => {
                                debug!("got single command {:?}", &command);
                                // we convert it to a sequence
                                Some(Sequence::new_single(command))
                            }
                            Ok(Message::Sequence(sequence)) => {
                                debug!("got sequence {:?}", &sequence);
                                Some(sequence)
                            }
                            Ok(message) => {
                                debug!("got something not yet handled: {:?}", message);
                                None
                            }
                            Err(e) => {
                                warn!("Read error : {:?}", e);
                                None
                            }
                        } {
                            if let Err(e) = tx.send(sequence) {
                                warn!("error while sending {:?}", e);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Stream error : {:?}", e);
                    }
                }
            }
        });
        Ok(Self { path })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        debug!("removing socket file");
        fs::remove_file(&self.path).unwrap();
    }
}
