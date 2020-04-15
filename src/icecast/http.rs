use std::str;

use derive_more::From;
use httparse::Request;
use tokio::io::AsyncReadExt;

use crate::listen::PeekTcpStream;

#[derive(Debug, From)]
pub enum Error {
    Io(tokio::io::Error),
    Http(httparse::Error),
    Eof,
    HeadersTooLong,
    NoPath,
    NoContentType,
}

#[derive(Debug)]
pub enum ContentType {
    Ogg,
}

#[derive(Debug)]
pub struct RequestInfo {
    pub path: String,
    pub content_type: Option<ContentType>,
    pub stream_data: Vec<u8>,
}

pub async fn parse(stream: &mut PeekTcpStream) -> Result<RequestInfo, Error> {
    let mut buff = [0u8; 4096];
    let mut buff_offset = 0;

    loop {
        if buff_offset == buff.len() {
            return Err(Error::HeadersTooLong);
        }

        let bytes = stream.read(&mut buff[buff_offset..]).await
            .map_err(Error::Io)?;

        if bytes == 0 {
            return Err(Error::Eof);
        }

        buff_offset += bytes;

        let mut headers = [httparse::EMPTY_HEADER; 32];
        let mut request = Request::new(&mut headers);

        match request.parse(&buff[0..buff_offset])? {
            httparse::Status::Partial => {}
            httparse::Status::Complete(len) => {
                return on_complete(&request, &buff[len..buff_offset]);
            }
        }
    }

    fn on_complete(request: &Request, stream_data: &[u8]) -> Result<RequestInfo, Error> {
        let content_type_hdr = request.headers.iter()
            .find(|header| header.name.eq_ignore_ascii_case("content-type"))
            .and_then(|header| str::from_utf8(header.value).ok())
            .ok_or(Error::NoContentType)?;

        let content_type = match content_type_hdr {
            "application/ogg" | "audio/ogg" => Some(ContentType::Ogg),
            _ => None,
        };

        Ok(RequestInfo {
            path: request.path.ok_or(Error::NoPath)?.to_owned(),
            content_type,
            stream_data: stream_data.to_vec(),
        })
    }
}

