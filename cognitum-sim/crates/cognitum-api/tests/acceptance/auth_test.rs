//! Acceptance tests for authentication

use actix_web::{test, web, App};
use cognitum_api::configure_app;
use serde_json::json;

mod helpers;
use helpers::*;

#[actix_web::test]
async fn should_reject_request_without_api_key() {
    let state = create_test_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure_app),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/v1/simulations")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["error"]["code"], "unauthorized");
}

#[actix_web::test]
async fn should_reject_invalid_api_key() {
    let state = create_test_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure_app),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/v1/simulations")
        .insert_header(("Authorization", "Bearer sk_invalid"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["error"]["code"], "unauthorized");
}

#[actix_web::test]
async fn should_accept_valid_api_key() {
    let state = create_test_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure_app),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/v1/simulations")
        .insert_header(("Authorization", "Bearer sk_test_xxx"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn should_allow_health_check_without_auth() {
    let state = create_test_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure_app),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/health")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "healthy");
}
