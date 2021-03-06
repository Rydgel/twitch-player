extern crate hyper;
extern crate url;
extern crate serde;
extern crate serde_json;

use self::hyper::server;

use prelude::futures::{Future, future};
use prelude::http::{QueryParams, parse_query_params, streaming_response};
use twitch::types::{StreamIndex, Quality, Stream};
use twitch::utils::find_playlist;
use super::state::{State, index_cache::IndexError};

use std::rc::Rc;

type ApiRequest = server::Request;
type ApiResponse = server::Response;
type ApiError = hyper::Error;
type ApiFuture = Box<Future<Item = ApiResponse, Error = ApiError>>;

pub struct TwitchdApi {
    state: Rc<State>
}

impl TwitchdApi {
    pub fn new(state: &Rc<State>) -> Self {
        Self { state: Rc::clone(state) }
    }

    fn get_stream_index(&self, params: QueryParams) -> ApiFuture {
        match params.get("channel") {
            None          => respond(bad_request("Missing channel")),
            Some(channel) => {
                let response = self.state.index_cache.get(channel)
                    .map(json_response)
                    .or_else(|error| Ok(index_error_response(error)));

                Box::new(response)
            }
        }
    }

    fn get_video_stream(&self, params: QueryParams) -> ApiFuture {
        match params.get("channel") {
            None          => respond(bad_request("Missing channel")),
            Some(channel) => {
                let quality = params.get("quality")
                    .map(|raw_quality| Quality::from(raw_quality.clone()))
                    .unwrap_or_default();
                let stream = (channel.clone(), quality);

                if self.state.player_pool.is_playing(&stream) {
                    let (sink, response) = streaming_response();
                    self.state.player_pool.add_sink(&stream, sink);
                    respond(response)
                } else {
                    self.fetch_and_play(stream)
                }
            }
        }
    }

    fn fetch_and_play(&self, stream: Stream) -> ApiFuture {
        let (channel, quality) = stream.clone();

        let stream_response = {
            let state = Rc::clone(&self.state);
            move |index: StreamIndex| {
                match find_playlist(index, &quality) {
                    None                => not_found(),
                    Some(playlist) => {
                        let (sink, response) = streaming_response();
                        state.player_pool.add_player(stream, playlist, sink);
                        response
                    }
                }
            }
        };

        let response = self.state.index_cache.get(&channel)
            .map(stream_response)
            .or_else(|error| Ok(index_error_response(error)));

        Box::new(response)
    }
}

impl server::Service for TwitchdApi {
    type Request = ApiRequest;
    type Response = ApiResponse;
    type Error = ApiError;
    type Future = ApiFuture;

    fn call(&self, req: Self::Request) -> Self::Future {
        use self::hyper::Method::Get;

        let params = req.query()
            .map(parse_query_params)
            .unwrap_or_default();

        match (req.method(), req.path()) {
            (Get, "/stream_index") => self.get_stream_index(params),
            (Get, "/play")         => self.get_video_stream(params),
            // Default => 404
            _ => respond(not_found())
        }
    }
}

fn not_found() -> ApiResponse {
    ApiResponse::new()
        .with_status(hyper::StatusCode::NotFound)
}

fn bad_request(detail: &str) -> ApiResponse {
    ApiResponse::new()
        .with_status(hyper::StatusCode::BadRequest)
        .with_body(String::from(detail))
}

fn server_error(detail: &str) -> ApiResponse {
    ApiResponse::new()
        .with_status(hyper::StatusCode::InternalServerError)
        .with_body(String::from(detail))
}

fn json_response(value: impl serde::Serialize) -> ApiResponse {
    use self::serde_json::to_vec as encode;
    use self::hyper::{header, mime};

    let reply_with_data = |data: Vec<u8>| {
        ApiResponse::new()
            .with_header(header::ContentLength(data.len() as u64))
            .with_header(header::ContentType(mime::APPLICATION_JSON))
            .with_body(data)
    };

    let reply_with_error = |error| {
        let detail = format!("Encoding error: {}", error);
        ApiResponse::new()
            .with_status(hyper::StatusCode::InternalServerError)
            .with_body(detail)
    };

    encode(&value)
        .map(reply_with_data)
        .unwrap_or_else(reply_with_error)
}

fn index_error_response(error: IndexError) -> ApiResponse {
    match error {
        IndexError::NotFound          => not_found(),
        IndexError::Unexpected(error) => server_error(&error)
    }
}

fn respond(response: ApiResponse) -> ApiFuture {
    Box::new(future::ok(response))
}
