use axum::{ http::{ header, StatusCode}, response::{ IntoResponse, Response}};
use serde::{ Serialize, Serializer};
use std::fmt::Display;
use validator::{ ValidationError, ValidationErrors};

pub trait Context {
    type Output;

    fn context<C: ToString>(self, context: C) -> Self::Output;
}

pub trait NestedValidation {
    type Output;

    fn nest_validation(self, parent: &'static str) -> Self::Output;
}

/// Unified API error.
#[derive(Serialize)]
pub struct Error {
    #[serde(serialize_with = "Error::serialize_status")]
    status: StatusCode,
    text: Option<String>,
    context: Option<String>,
    validation: Option<ValidationErrors>,
}

impl Error {
    pub fn new<E: Display>(status: StatusCode, err: Option<E>) -> Self {
        Self {
            validation: None,
            text: err.map(|e| format!("{:#}", e)),
            context: None,
            status,
        }
    }

    pub fn validation(errs: ValidationErrors) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            validation: Some(errs),
            text: None,
            context: None,
        }
    }

    pub fn validation_custom(field: &'static str, err: &'static str) -> Self {
        let mut container = ValidationErrors::new();
        container.add(field, ValidationError::new(err));
        Self::validation(container)
    }

    pub fn validation_required(field: &'static str) -> Self {
        Self::validation_custom(field, "required")
    }

    pub fn validation_unique(field: &'static str) -> Self {
        Self::validation_custom(field, "must be unique")
    }

    pub fn validation_exists(field: &'static str) -> Self {
        Self::validation_custom(field, "already exists")
    }

    pub fn validation_not_exists(field: &'static str) -> Self {
        Self::validation_custom(field, "not exists")
    }

    pub fn c500<E: Display>(err: E) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, Some(err))
    }

    pub fn c406<E: Display>(err: E) -> Self {
        Self::new(StatusCode::NOT_ACCEPTABLE, Some(err))
    }

    pub fn c404<E: Display>(err: E) -> Self {
        Self::new(StatusCode::NOT_FOUND, Some(err))
    }

    pub fn c401() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, Some("unauthorized"))
    }

    pub fn c403() -> Self {
        Self::new(StatusCode::FORBIDDEN, Some("forbidden"))
    }

    pub fn c400<E: Display>(err: E) -> Self {
        Self::new(StatusCode::BAD_REQUEST, Some(err))
    }

    pub fn c410() -> Self {
        Self::new(StatusCode::GONE, Some("disabled"))
    }

    pub fn cannot_be_deleted_constraint() -> Self {
        Self::new(
            StatusCode::NOT_ACCEPTABLE,
            Some("Сущность не может быть удалена, поскольку на нее ссылаются другие сущности"),
        )
    }

    pub fn cannot_be_deleted_children() -> Self {
        Self::new(
            StatusCode::NOT_ACCEPTABLE,
            Some("Сущность не может быть удалена, поскольку она содержит дочерние элементы"),
        )
    }

    pub fn with_context<T: ToString>(mut self, c: T) -> Self {
        self.context = Some(c.to_string());
        self
    }

    fn serialize_status<S: Serializer>(status: &StatusCode, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u16(status.as_u16())
    }
}

impl<V> Context for Result<V, Error> {
    type Output = Self;
    fn context<C: ToString>(self, c: C) -> Self::Output {
        self.map_err(|e| e.with_context(c))
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let body = serde_json::to_string_pretty(&self).unwrap();
        let header = [(header::CONTENT_TYPE, "application/json")];
        (self.status, header, body).into_response()
    }
}

impl From<StatusCode> for Error {
    fn from(status: StatusCode) -> Self {
        Self::new(status, Some(status.to_string()))
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap())
    }
}

impl<V> NestedValidation for Result<V, Error> {
    type Output = Self;
    fn nest_validation(self, parent: &'static str) -> Self::Output {
        self.map_err(|mut e| {
            e.validation = e
                .validation
                .map(|v| ValidationErrors::merge(Ok(()), parent, Err(v)).unwrap_err());
            e
        })
    }
}
