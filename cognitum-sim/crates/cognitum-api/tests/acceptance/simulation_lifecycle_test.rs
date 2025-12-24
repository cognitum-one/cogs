//! Acceptance tests for simulation lifecycle

use actix_web::{test, web, App};
use cognitum_api::{configure_app, models::AppState};
use serde_json::json;

mod helpers;
use helpers::*;

#[actix_web::test]
async fn should_create_simulation() {
    // Given: API server with mocked services
    let state = create_test_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure_app),
    )
    .await;

    // When: Creating simulation
    let req = test::TestRequest::post()
        .uri("/api/v1/simulations")
        .set_json(json!({
            "config": {
                "tiles": 16,
                "memory_per_tile": 156000,
                "enable_crypto": true
            },
            "program_id": "prog_test123"
        }))
        .insert_header(("Authorization", "Bearer sk_test_xxx"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Then: Returns 201 Created
    assert_eq!(resp.status(), 201);
}

#[actix_web::test]
async fn should_run_simulation_and_get_results() {
    let state = create_test_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure_app),
    )
    .await;

    // Create simulation
    let create_req = test::TestRequest::post()
        .uri("/api/v1/simulations")
        .set_json(json!({
            "config": {
                "tiles": 16,
                "memory_per_tile": 156000
            },
            "program_id": "prog_test123"
        }))
        .insert_header(("Authorization", "Bearer sk_test_xxx"))
        .to_request();

    let create_resp = test::call_service(&app, create_req).await;
    assert_eq!(create_resp.status(), 201);

    let body: serde_json::Value = test::read_body_json(create_resp).await;
    let sim_id = body["id"].as_str().unwrap();

    // Run simulation
    let run_req = test::TestRequest::post()
        .uri(&format!("/api/v1/simulations/{}/run", sim_id))
        .set_json(json!({ "cycles": 10000 }))
        .insert_header(("Authorization", "Bearer sk_test_xxx"))
        .to_request();

    let run_resp = test::call_service(&app, run_req).await;
    assert_eq!(run_resp.status(), 202);

    // Get results
    let results_req = test::TestRequest::get()
        .uri(&format!("/api/v1/simulations/{}/results", sim_id))
        .insert_header(("Authorization", "Bearer sk_test_xxx"))
        .to_request();

    let results_resp = test::call_service(&app, results_req).await;
    assert_eq!(results_resp.status(), 200);
}

#[actix_web::test]
async fn should_delete_simulation() {
    let state = create_test_state();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(configure_app),
    )
    .await;

    // Create simulation
    let create_req = test::TestRequest::post()
        .uri("/api/v1/simulations")
        .set_json(json!({
            "config": {
                "tiles": 16,
                "memory_per_tile": 156000
            },
            "program_id": "prog_test123"
        }))
        .insert_header(("Authorization", "Bearer sk_test_xxx"))
        .to_request();

    let create_resp = test::call_service(&app, create_req).await;
    let body: serde_json::Value = test::read_body_json(create_resp).await;
    let sim_id = body["id"].as_str().unwrap();

    // Delete simulation
    let delete_req = test::TestRequest::delete()
        .uri(&format!("/api/v1/simulations/{}", sim_id))
        .insert_header(("Authorization", "Bearer sk_test_xxx"))
        .to_request();

    let delete_resp = test::call_service(&app, delete_req).await;
    assert_eq!(delete_resp.status(), 204);

    // Verify deleted
    let get_req = test::TestRequest::get()
        .uri(&format!("/api/v1/simulations/{}", sim_id))
        .insert_header(("Authorization", "Bearer sk_test_xxx"))
        .to_request();

    let get_resp = test::call_service(&app, get_req).await;
    assert_eq!(get_resp.status(), 404);
}
