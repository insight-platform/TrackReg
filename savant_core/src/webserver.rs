pub mod kvs;
mod kvs_handlers;

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::get_or_init_async_runtime;
use crate::metrics::metric_collector::SystemMetricCollector;
use crate::metrics::pipeline_metric_builder::PipelineMetricBuilder;
use crate::pipeline::implementation;
use crate::primitives::Attribute;
use crate::webserver::kvs_handlers::{
    delete_handler, delete_single_handler, get_handler, search_handler, search_keys_handler,
    set_handler, set_handler_ttl,
};
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use lazy_static::lazy_static;
use log::{debug, error, info};
use moka::future::Cache;
use moka::Expiry;
use prometheus_client::encoding::text::encode;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

struct RecordExpiration;

impl Expiry<(String, String), (Option<u64>, Attribute)> for RecordExpiration {
    fn expire_after_create(
        &self,
        _: &(String, String),
        value: &(Option<u64>, Attribute),
        _created_at: Instant,
    ) -> Option<Duration> {
        value.0.map(Duration::from_millis)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum PipelineStatus {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "shutdown")]
    Shutdown,
}

const MAX_TTL_KVS_CAPACITY: u64 = 100_000;

#[allow(clippy::type_complexity)]
struct WsData {
    pipelines: Arc<Mutex<Vec<Arc<implementation::Pipeline>>>>,
    status: Arc<Mutex<PipelineStatus>>,
    shutdown_token: Arc<OnceLock<String>>,
    shutdown_status: Arc<OnceLock<bool>>,
    kvs: Arc<Cache<(String, String), (Option<u64>, Attribute)>>,
}

impl WsData {
    pub fn new() -> Self {
        let cache = Cache::builder()
            .max_capacity(MAX_TTL_KVS_CAPACITY)
            .expire_after(RecordExpiration {})
            .build();
        WsData {
            pipelines: Arc::new(Mutex::new(Vec::new())),
            status: Arc::new(Mutex::new(PipelineStatus::Stopped)),
            shutdown_token: Arc::new(OnceLock::new()),
            shutdown_status: Arc::new(OnceLock::new()),
            kvs: Arc::new(cache),
        }
    }

    pub fn set_status(&self, s: PipelineStatus) -> anyhow::Result<()> {
        let runtime = get_or_init_async_runtime();
        let thread_status = self.status.clone();
        let ws_job = WS_JOB
            .get()
            .ok_or_else(|| anyhow::anyhow!("Web server job not started"))?;
        if ws_job.is_finished() {
            error!("Web server job is finished unexpectedly, cannot update status.");
        }
        runtime.spawn(async move {
            let mut bind = thread_status.lock().await;
            *bind = s;
        });
        Ok(())
    }

    pub fn set_shutdown_token(&self, token: String) {
        let runtime = get_or_init_async_runtime();
        let thread_token = self.shutdown_token.clone();
        runtime.spawn(async move {
            let val = thread_token.get_or_init(|| token.clone());
            if val != &token {
                error!("Attempted to set shutdown token to a different value.");
            }
        });
    }
}

static WS_JOB: OnceLock<JoinHandle<()>> = OnceLock::new();

lazy_static! {
    static ref WS_DATA: web::Data<WsData> = web::Data::new(WsData::new());
    static ref PID: Mutex<i32> = Mutex::new(0);
}

pub(crate) fn register_pipeline(pipeline: Arc<implementation::Pipeline>) {
    let runtime = get_or_init_async_runtime();
    let stats = WS_DATA.pipelines.clone();
    runtime.block_on(async move {
        let mut bind = stats.lock().await;
        bind.push(pipeline);
        info!("Pipeline registered in stats.");
    });
}

pub(crate) fn unregister_pipeline(pipeline: Arc<implementation::Pipeline>) {
    let runtime = get_or_init_async_runtime();
    let stats = WS_DATA.pipelines.clone();
    runtime.block_on(async move {
        let mut bind = stats.lock().await;
        let prev_len = bind.len();
        debug!("Removing pipeline from stats.");
        bind.retain(|p| !Arc::ptr_eq(p, &pipeline));
        if bind.len() == prev_len {
            error!("Failed to remove pipeline from stats.");
        }
    });
}

pub(crate) async fn get_registered_pipelines() -> Vec<Arc<implementation::Pipeline>> {
    let s = WS_DATA.pipelines.lock().await;
    s.clone()
}

pub fn set_status(s: PipelineStatus) -> anyhow::Result<()> {
    WS_DATA.set_status(s)
}

pub async fn get_status() -> PipelineStatus {
    let s = WS_DATA.status.lock().await;
    s.clone()
}

pub fn set_shutdown_token(token: String) {
    WS_DATA.set_shutdown_token(token);
}

fn get_shutdown_token() -> Option<String> {
    WS_DATA.shutdown_token.get().cloned()
}

pub fn is_shutdown_set() -> bool {
    WS_DATA.shutdown_status.get().cloned().unwrap_or(false)
}

#[cfg(test)]
fn shutdown() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(test))]
fn shutdown() -> anyhow::Result<()> {
    WS_DATA
        .shutdown_status
        .set(true)
        .map_err(|_| anyhow::anyhow!("Shutdown status already set"))?;
    Ok(())
}

#[get("/status")]
async fn status_handler() -> impl Responder {
    let s = get_status().await;
    HttpResponse::Ok().json(s)
}

#[derive(Deserialize)]
enum ShutdownMode {
    #[serde(rename = "graceful")]
    Notify,
    #[serde(rename = "signal")]
    Signal,
}

#[derive(Deserialize)]
struct ShutdownParams {
    token: String,
    mode: ShutdownMode,
}

static SHUTDOWN_SIGNAL_NO: OnceLock<nix::sys::signal::Signal> = OnceLock::new();

fn get_shutdown_signal() -> nix::sys::signal::Signal {
    let signal = SHUTDOWN_SIGNAL_NO.get_or_init(|| nix::sys::signal::Signal::SIGINT);
    *signal
}

pub fn set_shutdown_signal(signal: i32) -> anyhow::Result<()> {
    let signal = nix::sys::signal::Signal::try_from(signal)
        .map_err(|e| anyhow::anyhow!("Invalid signal number: {}", e))?;
    SHUTDOWN_SIGNAL_NO
        .set(signal)
        .map_err(|s| anyhow::anyhow!("Signal already set: {}", s))
}

#[post("/shutdown/{token}/{mode}")]
async fn shutdown_handler(params: web::Path<ShutdownParams>) -> HttpResponse {
    let shutdown_params: ShutdownParams = params.into_inner();
    let shutdown_token = get_shutdown_token();
    if shutdown_token.is_none() {
        return HttpResponse::InternalServerError()
            .body("No shutdown token set. Pipeline shutdown is not supported.");
    } else if shutdown_token.unwrap() != shutdown_params.token {
        return HttpResponse::Unauthorized()
            .body("Invalid shutdown token provided (ignoring the command).");
    } else {
        let res = shutdown();
        if res.is_err() {
            return HttpResponse::InternalServerError()
                .body("Failed to set shutdown status multiple times (already set).");
        }
        let res = set_status(PipelineStatus::Shutdown);
        if res.is_err() {
            return HttpResponse::InternalServerError().body("Failed to set pipeline status.");
        }
        if matches!(shutdown_params.mode, ShutdownMode::Signal) {
            let pid = PID.lock().await;
            _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(*pid), get_shutdown_signal());
        }
    }
    HttpResponse::Ok().json("ok")
}

#[get("/metrics")]
async fn metrics_handler() -> HttpResponse {
    let content_type = "application/openmetrics-text; version=1.0.0; charset=utf-8";
    if let Err(e) = PipelineMetricBuilder::build().await {
        error!("Failed to build pipeline metrics: {}", e);
        return HttpResponse::InternalServerError()
            .content_type(content_type)
            .body("Failed to build pipeline metrics");
    }
    let mut registry = prometheus_client::registry::Registry::default();
    let boxed_collector = Box::new(SystemMetricCollector);
    registry.register_collector(boxed_collector);
    let mut body = String::new();
    if let Err(e) = encode(&mut body, &registry) {
        error!("Failed to encode metrics: {}", e);
        return HttpResponse::InternalServerError()
            .content_type(content_type)
            .body("Failed to encode metrics");
    }
    HttpResponse::Ok().content_type(content_type).body(body)
}

pub fn init_webserver(port: u16) -> anyhow::Result<()> {
    let pid = std::process::id() as i32;
    let rt = get_or_init_async_runtime();
    rt.block_on(async {
        let mut bind = PID.lock().await;
        *bind = pid;
    });

    if WS_JOB.get().is_some() {
        return Ok(());
    }
    let job_id = rt.spawn(async move {
        HttpServer::new(move || {
            App::new()
                .service(status_handler)
                .service(shutdown_handler)
                .service(metrics_handler)
                .service(set_handler)
                .service(set_handler_ttl)
                .service(delete_handler)
                .service(delete_single_handler)
                .service(search_handler)
                .service(get_handler)
                .service(search_keys_handler)
        })
        .bind(("0.0.0.0", port))
        .expect("Failed to bind to host:port")
        .run()
        .await
        .expect("Failed to run server");
        error!("Status web server stopped unexpectedly.");
    });
    WS_JOB.get_or_init(|| job_id);
    Ok(())
}

pub fn stop_webserver() {
    let ws_job = WS_JOB.get().expect("Web server job not started");
    ws_job.abort();
}

#[cfg(test)]
mod tests {
    use crate::get_or_init_async_runtime;
    use crate::metrics::{
        delete_metric_family, get_or_create_counter_family, get_or_create_gauge_family,
        set_extra_labels,
    };
    use crate::pipeline::implementation::create_test_pipeline;
    use crate::primitives::attribute_set::AttributeSet;
    use crate::primitives::Attribute;
    use crate::protobuf::{from_pb, ToProtobuf};
    use crate::test::gen_frame;
    use crate::webserver::kvs::synchronous::get_attribute;
    use crate::webserver::kvs::synchronous::set_attributes;
    use crate::webserver::{
        init_webserver, register_pipeline, set_shutdown_token, set_status, stop_webserver,
        PipelineStatus,
    };
    use hashbrown::HashMap;
    use prometheus_client::registry::Unit;
    use savant_protobuf::generated;
    use std::sync::Arc;
    use std::thread::sleep;
    use std::time::{Duration, SystemTime};

    const TOKEN: &str = "12345";

    #[test]
    #[serial_test::serial]
    fn test_attributes_abi_to_api() -> anyhow::Result<()> {
        init_webserver(8888)?;
        sleep(Duration::from_millis(100));
        set_status(PipelineStatus::Running)?;
        let ttl_attribute_set = vec![Attribute::persistent("jkl", "yay", vec![], &None, false)];
        let attribute_set = vec![Attribute::persistent("ghi", "yay", vec![], &None, false)];
        set_attributes(&ttl_attribute_set, Some(1000));
        set_attributes(&attribute_set, None);

        let r = reqwest::blocking::get("http://localhost:8888/kvs/search-keys/*/*")?;
        assert_eq!(r.status(), 200);
        let mut result: Vec<(String, String)> = r.json()?;
        result.sort_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        assert_eq!(
            result,
            vec![
                ("ghi".to_string(), "yay".to_string()),
                ("jkl".to_string(), "yay".to_string())
            ]
        );
        sleep(Duration::from_millis(1001));

        let r = reqwest::blocking::get("http://localhost:8888/kvs/search-keys/*/*")?;
        assert_eq!(r.status(), 200);
        let result: Vec<(String, String)> = r.json()?;
        assert_eq!(result, vec![("ghi".to_string(), "yay".to_string())]);

        let r = reqwest::blocking::get("http://localhost:8888/kvs/search/*/*")?;
        assert_eq!(r.status(), 200);
        let binary = r.bytes()?;
        let res_attribute_set = from_pb::<generated::AttributeSet, AttributeSet>(&binary)?;
        assert_eq!(res_attribute_set.attributes, attribute_set);

        let r = reqwest::blocking::get("http://localhost:8888/kvs/get/ghi/yay")?;
        assert_eq!(r.status(), 200);
        let binary = r.bytes()?;
        let res_attribute = from_pb::<generated::AttributeSet, AttributeSet>(&binary)?.attributes;
        assert_eq!(res_attribute[0], attribute_set[0]);

        let rt = get_or_init_async_runtime();
        let client = reqwest::Client::new();
        // delete single
        let r = rt.block_on(async {
            let resp = client
                .post("http://localhost:8888/kvs/delete-single/ghi/yay")
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            resp.bytes().await
        })?;
        let res_attribute = from_pb::<generated::AttributeSet, AttributeSet>(&r)?.attributes;
        assert_eq!(res_attribute[0], attribute_set[0]);

        // delete after delete
        let r = rt.block_on(async {
            let resp = client
                .post("http://localhost:8888/kvs/delete-single/ghi/yay")
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            resp.bytes().await
        })?;
        let res_attribute = from_pb::<generated::AttributeSet, AttributeSet>(&r)?.attributes;
        assert_eq!(res_attribute.len(), 0);

        // set again and purge
        set_attributes(&attribute_set, None);
        let r = rt.block_on(async {
            let resp = client
                .post("http://localhost:8888/kvs/delete/*/yay")
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            resp.bytes().await
        })?;
        assert_eq!(r.len(), 0); // returns nothing
                                // ensure that nothing exists with get
        let r = rt.block_on(async {
            let resp = client
                .get("http://localhost:8888/kvs/get/ghi/yay")
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            resp.bytes().await
        })?;
        assert_eq!(r.len(), 0);

        stop_webserver();
        Ok(())
    }

    #[test]
    #[serial_test::serial]
    fn test_api_to_abi() -> anyhow::Result<()> {
        init_webserver(8888)?;
        sleep(Duration::from_millis(100));
        set_status(PipelineStatus::Running)?;
        let ttl_attribute_set = AttributeSet::from(vec![Attribute::persistent(
            "jkl",
            "yay",
            vec![],
            &None,
            false,
        )]);

        let attribute_set = AttributeSet::from(vec![Attribute::persistent(
            "ghi",
            "yay",
            vec![],
            &None,
            false,
        )]);

        // set without ttl
        let rt = get_or_init_async_runtime();
        let client = reqwest::Client::new();

        let r = rt.block_on(async {
            let resp = client
                .post("http://localhost:8888/kvs/set")
                .body(attribute_set.to_pb().unwrap())
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            resp.bytes().await
        })?;
        assert_eq!(r.len(), 0);
        let attr = get_attribute(&"ghi".to_string(), &"yay".to_string());
        assert_eq!(attr.unwrap(), attribute_set.attributes[0]);

        // set with ttl
        let _ = rt.block_on(async {
            let resp = client
                .post("http://localhost:8888/kvs/set-with-ttl/1000")
                .body(ttl_attribute_set.to_pb().unwrap())
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), 200);
            resp.bytes().await
        })?;
        let attr = get_attribute(&"jkl".to_string(), &"yay".to_string());
        assert_eq!(attr.unwrap(), ttl_attribute_set.attributes[0]);
        sleep(Duration::from_millis(1001));
        let attr = get_attribute(&"jkl".to_string(), &"yay".to_string());
        assert!(attr.is_none());

        stop_webserver();
        Ok(())
    }

    #[test]
    #[serial_test::serial]
    fn test_webserver() -> anyhow::Result<()> {
        // unsafe {
        //     std::env::set_var("RUST_LOG", "debug");
        // }
        // _ = env_logger::try_init();
        init_webserver(8888)?;
        sleep(Duration::from_millis(100));
        set_status(PipelineStatus::Running)?;
        let r = reqwest::blocking::get("http://localhost:8888/status")?;
        assert_eq!(r.status(), 200);
        let s: PipelineStatus = r.json()?;
        assert!(matches!(s, PipelineStatus::Running));
        stop_webserver();
        Ok(())
    }

    #[test]
    #[serial_test::serial]
    fn test_webserver_shutdown_graceful() -> anyhow::Result<()> {
        // unsafe {
        //     std::env::set_var("RUST_LOG", "debug");
        // }
        // _ = env_logger::try_init();
        let rt = get_or_init_async_runtime();
        set_shutdown_token(TOKEN.to_string());
        init_webserver(8888)?;
        sleep(Duration::from_millis(500));
        set_status(PipelineStatus::Running)?;
        let client = reqwest::Client::new();
        let r = rt.block_on(
            client
                .post("http://localhost:8888/shutdown/12345/graceful")
                .send(),
        )?;
        assert_eq!(r.status(), 200);
        stop_webserver();
        Ok(())
    }

    #[test]
    #[serial_test::serial]
    fn test_webserver_shutdown_signal() -> anyhow::Result<()> {
        let rt = get_or_init_async_runtime();
        set_shutdown_token(TOKEN.to_string());
        init_webserver(8888)?;
        sleep(Duration::from_millis(500));
        set_status(PipelineStatus::Running)?;
        let (snd, rec) = crossbeam::channel::bounded(1);
        ctrlc::set_handler(move || {
            snd.send(()).unwrap();
        })
        .expect("Error setting Ctrl-C handler");
        let client = reqwest::Client::new();
        let r = rt.block_on(
            client
                .post("http://localhost:8888/shutdown/12345/signal")
                .send(),
        )?;
        rec.recv().unwrap();
        assert_eq!(r.status(), 200);
        stop_webserver();
        Ok(())
    }

    #[test]
    #[serial_test::serial]
    fn test_webserver_metrics() -> anyhow::Result<()> {
        // unsafe {
        //     std::env::set_var("RUST_LOG", "debug");
        // }
        // _ = env_logger::try_init();
        let pipeline = Arc::new(create_test_pipeline()?);
        pipeline.set_name("test_pipeline".into())?;
        register_pipeline(pipeline.clone());
        let id1 = pipeline.add_frame("input", gen_frame())?;
        // sleep for 5 ms
        sleep(Duration::from_millis(5));
        let id2 = pipeline.add_frame("input", gen_frame())?;
        // sleep for 3 ms
        sleep(Duration::from_millis(3));
        let batch_id = pipeline.move_and_pack_frames("proc1", vec![id1, id2])?;
        // sleep for 2 ms
        sleep(Duration::from_millis(2));
        pipeline.move_as_is("proc2", vec![batch_id])?;
        // sleep for 1 ms
        sleep(Duration::from_millis(1));
        let ids = pipeline.move_and_unpack_batch("output", batch_id)?;
        ids.iter().for_each(|id| {
            pipeline.delete(*id).unwrap();
        });

        let rt = get_or_init_async_runtime();
        set_shutdown_token(TOKEN.to_string());
        init_webserver(8888)?;
        sleep(Duration::from_millis(200));
        set_status(PipelineStatus::Running)?;
        set_extra_labels(HashMap::from([(
            String::from("hello"),
            String::from("there"),
        )]));

        let c = get_or_create_counter_family(
            "metric_counter",
            Some("Counter for metrics"),
            &["label1", "label2"],
            Some(Unit::Other(String::from("Number"))),
        );

        let g = get_or_create_gauge_family(
            "metric_gauge",
            Some("Gauge for metrics"),
            &["label3", "label3"],
            Some(Unit::Other(String::from("Time"))),
        );

        c.lock().inc(1, &[&"value1", &"value2"])?;
        let unix_time_now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();

        g.lock().set(unix_time_now, &[&"value3", &"value4"])?;

        let client = reqwest::Client::new();
        let r = rt.block_on(client.get("http://localhost:8888/metrics").send())?;
        assert_eq!(r.status(), 200);
        let text = rt.block_on(r.text())?;
        assert!(text.contains("metric_counter_Number_total"));
        assert!(text.contains("metric_gauge_Time"));
        assert!(text.contains("hello"));
        assert!(text.contains("stage_object_counter_total"));
        delete_metric_family("metric_counter");
        delete_metric_family("metric_gauge");
        stop_webserver();
        drop(pipeline);
        Ok(())
    }
}
