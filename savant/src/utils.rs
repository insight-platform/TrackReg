pub mod bbox;
pub mod byte_buffer;
pub mod eval_resolvers;
pub mod fps_meter;
pub mod otlp;
pub mod pluggable_udf_api;
pub mod python;
pub mod symbol_mapper;

use opentelemetry::global;
use opentelemetry::global::BoxedTracer;
use pyo3::prelude::*;

use crate::primitives::message::loader::{
    load_message_from_bytebuffer_gil, load_message_from_bytes_gil, load_message_py,
};
use crate::primitives::message::saver::{
    save_message_py, save_message_to_bytebuffer_gil, save_message_to_bytes_gil,
};

use crate::test::utils::{gen_empty_frame, gen_frame};
use crate::utils::symbol_mapper::RegistrationPolicy;
use crate::utils::symbol_mapper::{
    build_model_object_key_py, clear_symbol_maps_py, dump_registry_gil, get_model_id_py,
    get_model_name_py, get_object_id_py, get_object_ids_py, get_object_label_py,
    get_object_labels_py, is_model_registered_py, is_object_registered_py, parse_compound_key_py,
    register_model_objects_py, validate_base_key_py,
};

use crate::logging::{log_level_enabled, LogLevel};
use crate::primitives::bbox::transformations::VideoObjectBBoxTransformationProxy;
use crate::primitives::bbox::BBoxMetricType;
use crate::primitives::{Message, VideoObjectBBoxType};
use crate::utils::byte_buffer::ByteBuffer;
use crate::utils::otlp::{MaybeTelemetrySpan, PropagatedContext, TelemetrySpan};
use crate::utils::pluggable_udf_api::{
    call_object_inplace_modifier_gil, call_object_map_modifier_gil, call_object_predicate_gil,
    is_plugin_function_registered_py, register_plugin_function_py, UserFunctionType,
};
use crate::with_gil;
pub use fps_meter::FpsMeter;

#[pyfunction]
#[inline]
pub fn round_2_digits(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

pub fn get_tracer() -> BoxedTracer {
    global::tracer("video_pipeline")
}

/// When loglevel is set to Trace reports the number of nanoseconds spent waiting for the GIL
/// The report is sent to the current telemetry span
///
#[pyfunction]
pub fn estimate_gil_contention() {
    if log_level_enabled(LogLevel::Trace) {
        with_gil!(|_| {});
    }
}

#[pymodule]
pub fn symbol_mapper_module(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(build_model_object_key_py, m)?)?;
    m.add_function(wrap_pyfunction!(clear_symbol_maps_py, m)?)?;
    m.add_function(wrap_pyfunction!(dump_registry_gil, m)?)?;
    m.add_function(wrap_pyfunction!(get_model_id_py, m)?)?;
    m.add_function(wrap_pyfunction!(get_model_name_py, m)?)?;
    m.add_function(wrap_pyfunction!(get_object_id_py, m)?)?;
    m.add_function(wrap_pyfunction!(get_object_ids_py, m)?)?;
    m.add_function(wrap_pyfunction!(get_object_label_py, m)?)?;
    m.add_function(wrap_pyfunction!(get_object_labels_py, m)?)?;
    m.add_function(wrap_pyfunction!(is_model_registered_py, m)?)?;
    m.add_function(wrap_pyfunction!(is_object_registered_py, m)?)?;
    m.add_function(wrap_pyfunction!(parse_compound_key_py, m)?)?;
    m.add_function(wrap_pyfunction!(register_model_objects_py, m)?)?;
    m.add_function(wrap_pyfunction!(validate_base_key_py, m)?)?;

    m.add_class::<RegistrationPolicy>()?;

    Ok(())
}

#[pymodule]
pub fn udf_api_module(_py: Python, m: &PyModule) -> PyResult<()> {
    // UDF API
    m.add_function(wrap_pyfunction!(register_plugin_function_py, m)?)?;
    m.add_function(wrap_pyfunction!(is_plugin_function_registered_py, m)?)?;
    m.add_function(wrap_pyfunction!(call_object_predicate_gil, m)?)?;
    m.add_function(wrap_pyfunction!(call_object_inplace_modifier_gil, m)?)?;
    m.add_function(wrap_pyfunction!(call_object_map_modifier_gil, m)?)?;

    m.add_class::<UserFunctionType>()?;
    Ok(())
}

#[pymodule]
pub fn serialization_module(_py: Python, m: &PyModule) -> PyResult<()> {
    // ser deser
    m.add_function(wrap_pyfunction!(save_message_py, m)?)?;
    m.add_function(wrap_pyfunction!(save_message_to_bytebuffer_gil, m)?)?;
    m.add_function(wrap_pyfunction!(save_message_to_bytes_gil, m)?)?;

    m.add_function(wrap_pyfunction!(load_message_py, m)?)?;
    m.add_function(wrap_pyfunction!(load_message_from_bytebuffer_gil, m)?)?;
    m.add_function(wrap_pyfunction!(load_message_from_bytes_gil, m)?)?;

    m.add_class::<Message>()?;
    Ok(())
}

#[pymodule]
pub fn utils(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(gen_frame, m)?)?;
    m.add_function(wrap_pyfunction!(gen_empty_frame, m)?)?;
    // utility
    m.add_function(wrap_pyfunction!(round_2_digits, m)?)?;
    m.add_function(wrap_pyfunction!(estimate_gil_contention, m)?)?;

    m.add_class::<PropagatedContext>()?;
    m.add_class::<TelemetrySpan>()?;
    m.add_class::<MaybeTelemetrySpan>()?;
    m.add_class::<FpsMeter>()?;
    m.add_class::<ByteBuffer>()?;
    m.add_class::<VideoObjectBBoxType>()?;
    m.add_class::<VideoObjectBBoxTransformationProxy>()?;
    m.add_class::<BBoxMetricType>()?;

    Ok(())
}
