use crate::AppState;
use actix_web::{
    Error, HttpMessage, HttpResponse,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use futures_util::future::{LocalBoxFuture, Ready, ready};
use std::rc::Rc;
use std::sync::Arc; // Phụ thuộc vào khai báo AppState ở main.rs

pub struct ApiKeyAuth;

impl<S, B> Transform<S, ServiceRequest> for ApiKeyAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = ApiKeyAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ApiKeyAuthMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct ApiKeyAuthMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for ApiKeyAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let app_state = req
            .app_data::<actix_web::web::Data<Arc<AppState>>>()
            .unwrap();
        let expected_api_key = &app_state.api_key;

        let provided_key = req
            .headers()
            .get("X-API-Key")
            .and_then(|hv| hv.to_str().ok());

        if provided_key != Some(expected_api_key) {
            let response = HttpResponse::Unauthorized()
                .json(serde_json::json!({"error": "Unauthorized: Invalid or missing X-API-Key"}))
                .map_into_right_body();
            let (http_req, _payload) = req.into_parts();
            return Box::pin(ready(Ok(ServiceResponse::new(http_req, response))));
        }

        let srv = Rc::clone(&self.service);
        Box::pin(async move {
            let res = srv.call(req).await?;
            Ok(res.map_into_left_body())
        })
    }
}
