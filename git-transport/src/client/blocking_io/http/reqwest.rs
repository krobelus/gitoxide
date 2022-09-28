pub struct Remote {
    handle: Option<std::thread::JoinHandle<Result<(), reqwest::Error>>>,
    req: std::sync::mpsc::SyncSender<remote::Request>,
    res: std::sync::mpsc::Receiver<remote::Response>,
}

mod remote {
    use crate::client::http;
    use crate::client::http::reqwest::Remote;
    use git_features::io::pipe;
    use std::convert::TryFrom;
    use std::str::FromStr;

    /// initialization
    impl Remote {
        pub fn new() -> Self {
            let (req_send, req_recv) = std::sync::mpsc::sync_channel(0);
            let (_res_send, res_recv) = std::sync::mpsc::sync_channel(0);
            let handle = std::thread::spawn(move || -> Result<(), reqwest::Error> {
                for Request {
                    url: _,
                    headers: _,
                    upload: _,
                } in req_recv
                {
                    let _client = reqwest::blocking::ClientBuilder::new()
                        .connect_timeout(std::time::Duration::from_secs(20))
                        .build()?;
                }
                Ok(())
            });

            Remote {
                handle: Some(handle),
                req: req_send,
                res: res_recv,
            }
        }
    }

    /// utilities
    impl Remote {
        fn make_request(
            &mut self,
            url: &str,
            headers: impl IntoIterator<Item = impl AsRef<str>>,
            upload: bool,
        ) -> Result<http::PostResponse<pipe::Reader, pipe::Reader, pipe::Writer>, http::Error> {
            let mut header_map = reqwest::header::HeaderMap::new();
            for header_line in headers {
                let header_line = header_line.as_ref();
                let colon_pos = header_line
                    .find(':')
                    .expect("header line must contain a colon to separate key and value");
                let (header_name, value) = header_line.split_at(colon_pos);

                match reqwest::header::HeaderName::from_str(header_name)
                    .ok()
                    .zip(reqwest::header::HeaderValue::try_from(value.trim()).ok())
                {
                    Some((key, val)) => header_map.insert(key, val),
                    None => continue,
                };
            }
            if let Err(std::sync::mpsc::SendError(req)) = self.req.send(Request {
                url: url.to_owned(),
                headers: header_map,
                upload,
            }) {
                // maybe the previous operation left it downed, but the transport is reused.
                *self = Self::new();
                if self.req.send(req).is_err() {
                    let err = self
                        .handle
                        .take()
                        .expect("always present")
                        .join()
                        .expect("no panic")
                        .expect_err("no receiver means thread is down with error");
                    *self = Self::new();
                    return Err(http::Error::InitHttpClient { source: Box::new(err) });
                }
            }

            let Response {
                headers,
                body,
                upload_body,
            } = self.res.recv().expect("cannot fail between send and receive");

            Ok(http::PostResponse {
                post_body: upload_body,
                headers,
                body,
            })
        }
    }

    impl http::Http for Remote {
        type Headers = pipe::Reader;
        type ResponseBody = pipe::Reader;
        type PostBody = pipe::Writer;

        fn get(
            &mut self,
            url: &str,
            headers: impl IntoIterator<Item = impl AsRef<str>>,
        ) -> Result<http::GetResponse<Self::Headers, Self::ResponseBody>, http::Error> {
            self.make_request(url, headers, false).map(Into::into)
        }

        fn post(
            &mut self,
            url: &str,
            headers: impl IntoIterator<Item = impl AsRef<str>>,
        ) -> Result<http::PostResponse<Self::Headers, Self::ResponseBody, Self::PostBody>, http::Error> {
            self.make_request(url, headers, true)
        }
    }

    pub struct Request {
        pub url: String,
        pub headers: reqwest::header::HeaderMap,
        pub upload: bool,
    }
    pub struct Response {
        pub headers: pipe::Reader,
        pub body: pipe::Reader,
        pub upload_body: pipe::Writer,
    }
}
