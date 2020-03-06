use std::convert::TryFrom;
use std::fmt;
use std::str;

use http::request::Parts;

use hyper::header::{HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE};
use hyper::{Body, Error as HyperError, Response, StatusCode};

use json5;
use serde::{de, Serialize};
use serde_json;

pub enum BodyParseError {
    UnsupportedContentType(MimeType),
    UnsupportedCharset(String),
    RequestTooLarge(usize, usize),
    BodyReadingError(HyperError),
    ContentLengthMissing,
    EncodingError(str::Utf8Error),
    JsonError(serde_json::Error),
    Json5Error(json5::Error),
}

impl From<HyperError> for BodyParseError {
    fn from(e: HyperError) -> BodyParseError {
        BodyParseError::BodyReadingError(e)
    }
}
impl From<str::Utf8Error> for BodyParseError {
    fn from(e: str::Utf8Error) -> BodyParseError {
        BodyParseError::EncodingError(e)
    }
}
impl From<serde_json::Error> for BodyParseError {
    fn from(e: serde_json::Error) -> BodyParseError {
        BodyParseError::JsonError(e)
    }
}
impl From<json5::Error> for BodyParseError {
    fn from(e: json5::Error) -> BodyParseError {
        BodyParseError::Json5Error(e)
    }
}
impl Into<Response<Body>> for BodyParseError {
    fn into(self) -> Response<Body> {
        use BodyParseError::*;
        let (code, message) = match self {
            UnsupportedContentType(content_type) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!(
                    "unsupported content type '{}'. Try 'application/json'",
                    content_type
                ),
            ),
            UnsupportedCharset(charset) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!(
                    "unsupported charset '{}'. the request body must be charset=utf8",
                    charset
                ),
            ),
            RequestTooLarge(size, max_size) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "body must be not larger than {} bytes, was {}",
                    max_size, size
                ),
            ),
            ContentLengthMissing => (
                StatusCode::LENGTH_REQUIRED,
                String::from("Content-Length header must be specified"),
            ),
            BodyReadingError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                String::from("failed to read the request body"),
            ),
            EncodingError(utf8_error) => (
                StatusCode::BAD_REQUEST,
                format!(
                    "the request body was not parseable as UTF-8 at position {}",
                    utf8_error.valid_up_to()
                ),
            ),
            JsonError(e) => (
                StatusCode::BAD_REQUEST,
                format!("could not parse the json request: {}", e),
            ),
            Json5Error(e) => (
                StatusCode::BAD_REQUEST,
                format!("could not parse the json5 request: {}", e),
            ),
        };
        Response::builder()
            .status(code)
            .body(Body::from(message))
            .unwrap()
    }
}

/// Simple helpers to parse a content type string that may also contain a charset
struct ContentType {
    content_type: MimeType,
    charset_name: Option<String>,
}
impl From<&str> for ContentType {
    fn from(s: &str) -> ContentType {
        let mut parts = s.split(';');
        let content_type = MimeType::from(parts.next().unwrap()); // the first part is always present

        let charset_name = parts
            .map(|s| s.trim())
            .filter(|s| s.starts_with("charset="))
            .next()
            .map(|s| String::from(&s[8..]));
        ContentType {
            content_type,
            charset_name,
        }
    }
}
impl From<MimeType> for ContentType {
    fn from(m: MimeType) -> ContentType {
        ContentType {
            content_type: m,
            charset_name: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MimeType {
    mime_type: String,
    mime_subtype: String,
}
impl MimeType {
    fn is_compatible_with(&self, other: &MimeType) -> bool {
        Self::is_compatible_part(&self.mime_type, &other.mime_type)
            && Self::is_compatible_part(&self.mime_subtype, &other.mime_subtype)
    }

    fn is_compatible_part(part1: &str, part2: &str) -> bool {
        part1 == "*" || part2 == "*" || part1 == part2
    }

    fn json() -> MimeType {
        MimeType::from("application/json")
    }
    fn json5() -> MimeType {
        MimeType::from("application/json5")
    }
}
impl From<&str> for MimeType {
    fn from(s: &str) -> MimeType {
        let mut parts = s.split('/');
        let mime_type = String::from(parts.next().unwrap()); // OK to unwrap, first element on spliterator always exists
        let mime_subtype = String::from(parts.next().unwrap_or("*"));
        MimeType {
            mime_type,
            mime_subtype,
        }
    }
}
impl fmt::Display for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.mime_type, self.mime_subtype)
    }
}

pub enum SupportedMimeType {
    Json,
    Json5,
}
impl SupportedMimeType {
    fn from_mime_type(m: &MimeType) -> Result<SupportedMimeType, &MimeType> {
        use SupportedMimeType::*;
        match m {
            t if t == &MimeType::json() => Ok(Json),
            t if t == &MimeType::json5() => Ok(Json5),
            unsupported => Err(unsupported),
        }
    }

    fn from_mime_type_relaxed(m: &MimeType) -> Result<SupportedMimeType, &MimeType> {
        use SupportedMimeType::*;
        match m {
            t if t.is_compatible_with(&MimeType::json()) => Ok(Json),
            t if t.is_compatible_with(&MimeType::json5()) => Ok(Json5),
            unsupported => Err(unsupported),
        }
    }

    pub fn serialize<T: Serialize>(&self, x: &T) -> Body {
        use SupportedMimeType::*;
        match self {
            Json => Body::from(serde_json::ser::to_string(x).unwrap()),
            Json5 => Body::from(json5::to_string(x).unwrap()),
        }
    }

    fn deserialize<T: de::DeserializeOwned>(&self, s: &str) -> Result<T, BodyParseError> {
        use SupportedMimeType::*;
        match self {
            Json => Ok(serde_json::de::from_str(s)?),
            Json5 => Ok(json5::from_str(s)?),
        }
    }
}

/// Helper to work with the Accept http headers
struct Accept {
    mime_types: Vec<MimeType>,
}
impl Accept {
    fn is_empty(&self) -> bool {
        self.mime_types.is_empty()
    }
}
impl From<&str> for Accept {
    fn from(s: &str) -> Accept {
        Accept {
            mime_types: s
                .split(',')
                .map(|part| part.split(";").next().unwrap()) // unwrap is OK because at least 1 part will always exist
                .map(|s| MimeType::from(s))
                .collect(),
        }
    }
}
impl<'a> From<hyper::header::GetAll<'a, HeaderValue>> for Accept {
    fn from(headers: hyper::header::GetAll<'a, HeaderValue>) -> Accept {
        let accepts = headers
            .iter()
            .filter_map(|h| h.to_str().ok())
            .map(Accept::from);
        let mut all_mime_types: Vec<MimeType> = Vec::new();
        for mut accept in accepts {
            all_mime_types.append(&mut accept.mime_types);
        }
        Accept {
            mime_types: all_mime_types,
        }
    }
}
pub struct RichParts {
    method: http::Method,
    uri_path: String,
    content_type: ContentType,
    content_length: Option<usize>,
    accept: Accept,
    token: Option<String>,
}
impl From<&Parts> for RichParts {
    fn from(parts: &Parts) -> RichParts {
        let method = parts.method.clone();
        let uri_path = parts.uri.path().to_string();
        let content_type = Self::parse_content_type(parts);
        let content_length = Self::parse_content_length(parts);
        let accept = Self::parse_accept(parts);
        let token = Self::parse_token(parts);
        RichParts {
            method,
            uri_path,
            content_type: content_type,
            content_length,
            accept,
            token,
        }
    }
}
impl RichParts {
    fn get_content_type(&self) -> &ContentType {
        &self.content_type
    }

    fn get_content_length(&self) -> &Option<usize> {
        &self.content_length
    }

    pub fn guess_response_type(&self) -> Option<SupportedMimeType> {
        let explicit_accept_type = self
            .accept
            .mime_types
            .iter()
            .filter_map(|explicit_accept_type| {
                SupportedMimeType::from_mime_type_relaxed(explicit_accept_type).ok()
            })
            .next();
        // if the caller specified a supported accepy type, use that
        explicit_accept_type.or_else(|| {
            if !self.accept.is_empty() {
                // if the caller specified accept types, and none of them were supported, then do not attempt to
                // guess the response type
                None
            } else {
                // if the Content-Type was specified and supported, use that. Otherwise, fall back to the default.
                // `content_type` already does the default handling, so we don't have to repeat that here
                // (if the content type was specified and unsupported, we will likely not use the result of this
                // funciton anyway).
                SupportedMimeType::from_mime_type(&self.content_type.content_type).ok()
            }
        })
    }

    pub async fn deserialize_by_content_type<T: de::DeserializeOwned>(
        &self,
        body: Body,
        max_content_length: usize,
    ) -> Result<T, BodyParseError> {
        use BodyParseError::*;
        let content_type = self.get_content_type();

        // it is allowed for the Content-Type charset to be left unspecified. in that case we assume utf8. Values
        // other than utf8 are not allowed.
        match &content_type.charset_name.as_ref().filter(|c| c != &"utf8") {
            Some(unsupported_charset) => {
                return Err(UnsupportedCharset(unsupported_charset.to_string()))
            }
            None => (),
        };

        // check that the Content-Type is supported. (assuming application/json if unspecified)
        let content_type = SupportedMimeType::from_mime_type(&content_type.content_type).map_err(
            |unsupported_mime_type| UnsupportedContentType(unsupported_mime_type.clone()),
        )?;

        // check that the Content-Length is defined (mandatory) and that it does not exceed the max size the
        // handler is willing to process (this is to prevent DoS-type attacks. hyper will limit the body input stream
        // to Content-Length bytes).
        let content_length: usize = self.get_content_length().ok_or(ContentLengthMissing)?;
        if content_length > max_content_length {
            return Err(RequestTooLarge(content_length, max_content_length));
        }

        // read & deserialize the body
        let full_body = hyper::body::to_bytes(body).await?;
        let full_body_str = str::from_utf8(&full_body)?;
        content_type.deserialize(full_body_str)
    }

    pub fn does_match(&self, method: &http::Method, path: &str) -> bool {
        &self.method == method && self.uri_path == path
    }

    pub fn try_match<'a, 'b, T>(
        &'a self,
        method: &http::Method,
        path_pattern: &'b str,
    ) -> Result<T, UriMatchError>
    where
        T: TryFrom<Vec<&'a str>, Error = TupleWrapperParseError>,
    {
        use UriMatchError::*;
        if &self.method != method {
            return Err(MethodNotAllowed(method.clone()));
        }

        let mut path_pattern_parts = path_pattern.split('/');
        let mut path_parts = self.uri_path[..].split('/');
        let mut variables: Vec<&'a str> = Vec::new();

        loop {
            let next_pattern_part = path_pattern_parts.next();
            let next_path_part = path_parts.next();
            match (next_pattern_part, next_path_part) {
                (None, None) => break,
                (Some("{}"), Some(variable)) => variables.push(variable),
                (Some(pattern_part), Some(path_part)) if path_part == pattern_part => continue,
                _ => return Err(PathDoesNotMatch(self.uri_path.clone())),
            }
        }

        T::try_from(variables).map_err(|e| PartsParseError(e))
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_ref().map(|s| s.as_str())
    }

    // header parsing helpers

    fn parse_content_type(parts: &Parts) -> ContentType {
        parts
            .headers
            .get(CONTENT_TYPE)
            .iter()
            .next()
            .and_then(|h| h.to_str().ok())
            .map(|h| ContentType::from(h))
            .unwrap_or_else(|| ContentType::from(MimeType::json()))
    }

    fn parse_content_length(parts: &Parts) -> Option<usize> {
        parts
            .headers
            .get(CONTENT_LENGTH)
            .iter()
            .next()
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.parse().ok())
    }

    fn parse_accept(parts: &Parts) -> Accept {
        Accept::from(parts.headers.get_all(ACCEPT))
    }

    fn parse_token(parts: &Parts) -> Option<String> {
        parts
            .headers
            .get(AUTHORIZATION)
            .iter()
            .next()
            .and_then(|h| h.to_str().ok())
            .map(|h| h.to_string())
    }
}

// helpers for parsing URLs

pub enum UriMatchError {
    MethodNotAllowed(http::Method),
    PathDoesNotMatch(String),
    PartsParseError(TupleWrapperParseError),
}

pub enum TupleWrapperParseError {
    TooManyParams(usize),
    TooFewParams(usize),
    FailedToParsePart(usize),
}

pub struct TupleWrapper1<T>(pub T);
impl<'a, T1> TryFrom<Vec<&'a str>> for TupleWrapper1<T1>
where
    T1: str::FromStr,
{
    type Error = TupleWrapperParseError;

    fn try_from(s: Vec<&'a str>) -> Result<Self, Self::Error> {
        use TupleWrapperParseError::*;
        let l = s.len();
        match l {
            1 => {
                let inner = T1::from_str(s[0]).map_err(|_| FailedToParsePart(0))?;
                Ok(TupleWrapper1(inner))
            }
            too_few if l < 1 => Err(TooFewParams(too_few)),
            too_many => Err(TooManyParams(too_many)),
        }
    }
}

pub struct TupleWrapper2<T1, T2>(pub T1, pub T2);
impl<'a, T1, T2> TryFrom<Vec<&'a str>> for TupleWrapper2<T1, T2>
where
    T1: str::FromStr,
    T2: str::FromStr,
{
    type Error = TupleWrapperParseError;

    fn try_from(s: Vec<&'a str>) -> Result<Self, Self::Error> {
        use TupleWrapperParseError::*;
        let l = s.len();
        match l {
            2 => {
                let inner1 = T1::from_str(s[0]).map_err(|_| FailedToParsePart(0))?;
                let inner2 = T2::from_str(s[1]).map_err(|_| FailedToParsePart(1))?;
                Ok(TupleWrapper2(inner1, inner2))
            }
            too_few if l < 2 => Err(TooFewParams(too_few)),
            too_many => Err(TooManyParams(too_many)),
        }
    }
}