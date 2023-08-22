pub mod stage;

use crate::match_query::MatchQuery;
use crate::pipeline::PipelineStagePayloadType;
use crate::primitives::frame::VideoFrameProxy;
use crate::primitives::frame_batch::VideoFrameBatch;
use crate::primitives::frame_update::VideoFrameUpdate;
use crate::primitives::object::VideoObjectProxy;
use anyhow::Result;
use hashbrown::HashMap;
use opentelemetry::Context;
use std::sync::Arc;

#[derive(Clone, Default, Debug)]
pub struct Pipeline(Arc<implementation::Pipeline>);

impl Pipeline {
    pub fn new(stages: Vec<(String, PipelineStagePayloadType)>) -> Result<Self> {
        Ok(Self(Arc::new(implementation::Pipeline::new(stages)?)))
    }

    pub fn memory_handle(&self) -> usize {
        self as *const Self as usize
    }

    pub fn set_root_span_name(&self, name: String) -> Result<()> {
        self.0.set_root_span_name(name)
    }

    pub fn set_sampling_period(&self, period: i64) -> Result<()> {
        self.0.set_sampling_period(period)
    }

    pub fn get_sampling_period(&self) -> i64 {
        *self.0.get_sampling_period()
    }

    pub fn get_root_span_name(&self) -> String {
        self.0.get_root_span_name().clone()
    }

    pub fn get_stage_type(&self, name: &str) -> Option<PipelineStagePayloadType> {
        self.0.find_stage_type(name, 0)
    }

    pub fn add_frame_update(&self, frame_id: i64, update: VideoFrameUpdate) -> Result<()> {
        self.0.add_frame_update(frame_id, update)
    }

    pub fn add_batched_frame_update(
        &self,
        batch_id: i64,
        frame_id: i64,
        update: VideoFrameUpdate,
    ) -> Result<()> {
        self.0.add_batched_frame_update(batch_id, frame_id, update)
    }

    pub fn add_frame(&self, stage_name: &str, frame: VideoFrameProxy) -> Result<i64> {
        self.0.add_frame(stage_name, frame)
    }

    pub fn add_frame_with_telemetry(
        &self,
        stage_name: &str,
        frame: VideoFrameProxy,
        parent_ctx: Context,
    ) -> Result<i64> {
        self.0
            .add_frame_with_telemetry(stage_name, frame, parent_ctx)
    }

    pub fn delete(&self, id: i64) -> Result<HashMap<i64, Context>> {
        self.0.delete(id)
    }

    pub fn get_stage_queue_len(&self, stage: &str) -> Result<usize> {
        self.0.get_stage_queue_len(stage)
    }

    pub fn get_independent_frame(&self, frame_id: i64) -> Result<(VideoFrameProxy, Context)> {
        self.0.get_independent_frame(frame_id)
    }

    pub fn get_batched_frame(
        &self,
        batch_id: i64,
        frame_id: i64,
    ) -> Result<(VideoFrameProxy, Context)> {
        self.0.get_batched_frame(batch_id, frame_id)
    }

    pub fn get_batch(&self, batch_id: i64) -> Result<(VideoFrameBatch, HashMap<i64, Context>)> {
        self.0.get_batch(batch_id)
    }

    pub fn apply_updates(&self, id: i64) -> Result<()> {
        self.0.apply_updates(id)
    }

    pub fn clear_updates(&self, id: i64) -> Result<()> {
        self.0.clear_updates(id)
    }

    pub fn move_as_is(&self, dest_stage_name: &str, object_ids: Vec<i64>) -> Result<()> {
        self.0.move_as_is(dest_stage_name, object_ids)
    }

    pub fn move_and_pack_frames(&self, dest_stage_name: &str, frame_ids: Vec<i64>) -> Result<i64> {
        self.0.move_and_pack_frames(dest_stage_name, frame_ids)
    }

    pub fn move_and_unpack_batch(&self, dest_stage_name: &str, batch_id: i64) -> Result<Vec<i64>> {
        self.0.move_and_unpack_batch(dest_stage_name, batch_id)
    }

    pub fn access_objects(
        &self,
        frame_id: i64,
        query: &MatchQuery,
    ) -> Result<HashMap<i64, Vec<VideoObjectProxy>>> {
        self.0.access_objects(frame_id, query)
    }

    pub fn get_id_locations_len(&self) -> usize {
        self.0.get_id_locations_len()
    }
}

pub(super) mod implementation {
    use crate::get_tracer;
    use crate::match_query::MatchQuery;
    use crate::pipeline::{PipelinePayload, PipelineStagePayloadType};
    use crate::pipeline2::stage::PipelineStage;
    use crate::primitives::frame::VideoFrameProxy;
    use crate::primitives::frame_batch::VideoFrameBatch;
    use crate::primitives::frame_update::VideoFrameUpdate;
    use crate::primitives::object::VideoObjectProxy;
    use anyhow::{bail, Result};
    use hashbrown::HashMap;
    use opentelemetry::trace::{SpanBuilder, TraceContextExt, TraceId, Tracer};
    use opentelemetry::Context;
    use parking_lot::RwLock;
    use std::sync::atomic::Ordering;
    use std::sync::OnceLock;

    const DEFAULT_ROOT_SPAN_NAME: &str = "video_pipeline";

    #[derive(Debug, Default)]
    pub struct Pipeline {
        id_counter: std::sync::atomic::AtomicI64,
        frame_counter: std::sync::atomic::AtomicI64,
        root_spans: RwLock<HashMap<i64, Context>>,
        stages: Vec<PipelineStage>,
        frame_locations: RwLock<HashMap<i64, usize>>,
        sampling_period: OnceLock<i64>,
        root_span_name: OnceLock<String>,
    }

    impl Pipeline {
        fn add_stage(&mut self, name: String, stage_type: PipelineStagePayloadType) -> Result<()> {
            if self.find_stage(&name, 0).is_some() {
                bail!("Stage with name {} already exists", name)
            }
            self.stages.push(PipelineStage {
                stage_name: name,
                stage_type,
                payload: Default::default(),
            });
            Ok(())
        }

        pub fn new(stages: Vec<(String, PipelineStagePayloadType)>) -> Result<Self> {
            let mut pipeline = Self::default();
            for (name, stage_type) in stages {
                pipeline.add_stage(name, stage_type)?;
            }
            Ok(pipeline)
        }

        pub fn get_id_locations_len(&self) -> usize {
            self.frame_locations.read().len()
        }

        pub fn set_root_span_name(&self, name: String) -> Result<()> {
            self.root_span_name.set(name).map_err(|last| {
                anyhow::anyhow!(
                    "Root span name can only be set once. Current value: {}",
                    last
                )
            })
        }

        pub fn set_sampling_period(&self, period: i64) -> Result<()> {
            self.sampling_period.set(period).map_err(|last| {
                anyhow::anyhow!(
                    "Sampling period can only be set once. Current value: {}",
                    last
                )
            })
        }

        pub fn get_sampling_period(&self) -> &i64 {
            self.sampling_period.get_or_init(|| 0)
        }

        pub fn get_root_span_name(&self) -> &String {
            self.root_span_name
                .get_or_init(|| DEFAULT_ROOT_SPAN_NAME.to_owned())
        }

        fn get_stage_span(&self, id: i64, span_name: String) -> Context {
            let bind = self.root_spans.read();
            let ctx = bind.get(&id).unwrap();

            if ctx.span().span_context().trace_id() == TraceId::INVALID {
                return Context::default();
            }

            let span = get_tracer().build_with_context(SpanBuilder::from_name(span_name), ctx);
            Context::current_with_span(span)
        }

        pub(crate) fn get_nested_span(span_name: String, parent_ctx: &Context) -> Context {
            if parent_ctx.span().span_context().trace_id() == TraceId::INVALID {
                return Context::default();
            }

            let span =
                get_tracer().build_with_context(SpanBuilder::from_name(span_name), parent_ctx);
            Context::current_with_span(span)
        }

        pub fn find_stage_type(
            &self,
            name: &str,
            start_from: usize,
        ) -> Option<PipelineStagePayloadType> {
            self.find_stage(name, start_from)
                .map(|(_, stage)| stage.stage_type.clone())
        }

        pub fn add_frame_update(&self, frame_id: i64, update: VideoFrameUpdate) -> Result<()> {
            let cur_stage = self.get_stage_for_id(frame_id)?;
            self.stages[cur_stage].add_frame_update(frame_id, update)?;
            Ok(())
        }

        pub fn add_batched_frame_update(
            &self,
            batch_id: i64,
            frame_id: i64,
            update: VideoFrameUpdate,
        ) -> Result<()> {
            let stage = self.get_stage_for_id(batch_id)?;
            if let Some(stage) = self.stages.get(stage) {
                stage.add_batched_frame_update(batch_id, frame_id, update)
            } else {
                bail!("Stage not found")
            }
        }

        pub fn add_frame(&self, stage_name: &str, frame: VideoFrameProxy) -> Result<i64> {
            let sampling_period = self.get_sampling_period();
            let next_frame = self.frame_counter.load(Ordering::SeqCst) + 1;
            let ctx = if *sampling_period <= 0 || next_frame % *sampling_period != 0 {
                Context::default()
            } else {
                get_tracer().in_span(self.get_root_span_name().clone(), |cx| cx)
            };
            self.add_frame_with_telemetry(stage_name, frame, ctx)
        }

        fn find_stage(
            &self,
            stage_name: &str,
            start_from: usize,
        ) -> Option<(usize, &PipelineStage)> {
            self.stages[start_from..]
                .iter()
                .enumerate()
                .map(|(i, s)| (i + start_from, s))
                .find(|(_, s)| s.stage_name == stage_name)
        }

        pub fn add_frame_with_telemetry(
            &self,
            stage_name: &str,
            frame: VideoFrameProxy,
            parent_ctx: Context,
        ) -> Result<i64> {
            if matches!(
                self.find_stage_type(stage_name, 0),
                Some(PipelineStagePayloadType::Batch)
            ) {
                bail!("Stage does not accept batched frames")
            }

            self.frame_counter.fetch_add(1, Ordering::SeqCst);
            let id_counter = self.id_counter.fetch_add(1, Ordering::SeqCst) + 1;

            if parent_ctx.span().span_context().trace_id() == TraceId::INVALID {
                self.root_spans
                    .write()
                    .insert(id_counter, Context::default());
            } else {
                let span = get_tracer().build_with_context(
                    SpanBuilder::from_name(self.get_root_span_name().clone()),
                    &parent_ctx,
                );

                self.root_spans
                    .write()
                    .insert(id_counter, Context::current_with_span(span));
            }

            let ctx = self.get_stage_span(id_counter, format!("add/{}", stage_name));
            let frame_payload = PipelinePayload::Frame(frame, Vec::new(), ctx);

            if let Some((index, stage)) = self.find_stage(stage_name, 0) {
                stage.add_frame_payload(id_counter, frame_payload)?;
                self.frame_locations.write().insert(id_counter, index);
            } else {
                bail!("Stage not found")
            }

            Ok(id_counter)
        }

        pub fn delete(&self, id: i64) -> Result<HashMap<i64, Context>> {
            let stage = self
                .frame_locations
                .write()
                .remove(&id)
                .ok_or(anyhow::anyhow!("Object location not found"))?;

            if let Some(stage) = self.stages.get(stage) {
                let removed = stage.delete(id);
                if removed.is_none() {
                    bail!("Object not found in stage")
                }

                let mut bind = self.root_spans.write();
                match removed.unwrap() {
                    PipelinePayload::Frame(_, _, ctx) => {
                        ctx.span().end();
                        let root_ctx = bind.remove(&id).unwrap();
                        Ok(HashMap::from([(id, root_ctx)]))
                    }
                    PipelinePayload::Batch(_, _, contexts) => Ok({
                        let mut bind = self.root_spans.write();
                        contexts
                            .into_iter()
                            .map(|(id, ctx)| {
                                ctx.span().end();
                                let root_ctx = bind.remove(&id).unwrap();
                                (id, root_ctx)
                            })
                            .collect()
                    }),
                }
            } else {
                bail!("Stage not found")
            }
        }

        pub fn get_stage_queue_len(&self, stage: &str) -> Result<usize> {
            if let Some((_, stage)) = self.find_stage(stage, 0) {
                Ok(stage.len())
            } else {
                bail!("Stage not found")
            }
        }

        fn get_stage_for_id(&self, id: i64) -> Result<usize> {
            let bind = self.frame_locations.read();
            if let Some(stage) = bind.get(&id) {
                Ok(*stage)
            } else {
                bail!("Object location not found")
            }
        }

        fn get_stages_for_ids(&self, ids: &[i64]) -> Result<Vec<(i64, usize)>> {
            let bind = self.frame_locations.read();
            let mut results = Vec::with_capacity(ids.len());
            for id in ids {
                let val = bind.get(id);
                if val.is_none() {
                    bail!("Object location not found for {}", id)
                }
                results.push((*id, *val.unwrap()));
            }
            Ok(results)
        }

        pub fn get_independent_frame(&self, frame_id: i64) -> Result<(VideoFrameProxy, Context)> {
            let stage = self.get_stage_for_id(frame_id)?;
            if let Some(stage) = self.stages.get(stage) {
                stage.get_independent_frame(frame_id)
            } else {
                bail!("Stage not found")
            }
        }

        pub fn get_batched_frame(
            &self,
            batch_id: i64,
            frame_id: i64,
        ) -> Result<(VideoFrameProxy, Context)> {
            let stage = self.get_stage_for_id(batch_id)?;
            if let Some(stage) = self.stages.get(stage) {
                stage.get_batched_frame(batch_id, frame_id)
            } else {
                bail!("Stage not found")
            }
        }

        pub fn get_batch(&self, batch_id: i64) -> Result<(VideoFrameBatch, HashMap<i64, Context>)> {
            let stage = self.get_stage_for_id(batch_id)?;
            if let Some(stage) = self.stages.get(stage) {
                stage.get_batch(batch_id)
            } else {
                bail!("Stage not found")
            }
        }

        pub fn apply_updates(&self, id: i64) -> Result<()> {
            let stage = self.get_stage_for_id(id)?;
            if let Some(stage) = self.stages.get(stage) {
                stage.apply_updates(id)
            } else {
                bail!("Stage not found")
            }
        }

        pub fn clear_updates(&self, id: i64) -> Result<()> {
            let stage = self.get_stage_for_id(id)?;
            if let Some(stage) = self.stages.get(stage) {
                stage.clear_updates(id)
            } else {
                bail!("Stage not found")
            }
        }

        fn update_frame_locations(&self, ids: &[i64], index: usize) {
            self.frame_locations
                .write()
                .extend(ids.iter().map(|id| (*id, index)));
        }

        fn check_ids_in_the_same_stage(&self, ids: &[i64]) -> Result<usize> {
            if ids.is_empty() {
                bail!("Object IDs cannot be empty")
            }

            let mut stages = self
                .get_stages_for_ids(ids)?
                .into_iter()
                .map(|(_, name)| name);

            let stage = stages.next().unwrap();

            for current_stage in stages {
                if current_stage != stage {
                    bail!("All objects must be in the same stage")
                }
            }
            Ok(stage)
        }

        pub fn move_as_is(&self, dest_stage_name: &str, object_ids: Vec<i64>) -> Result<()> {
            let source_index = self.check_ids_in_the_same_stage(&object_ids)?;
            let source_stage_opt = self.stages.get(source_index);
            if source_stage_opt.is_none() {
                bail!("Source stage not found")
            }
            let source_stage = source_stage_opt.unwrap();

            let dest_stage_opt = self.find_stage(dest_stage_name, source_index);
            if dest_stage_opt.is_none() {
                bail!("Destination stage not found")
            }

            let (dest_index, dest_stage) = dest_stage_opt.unwrap();

            if source_stage.stage_type != dest_stage.stage_type {
                bail!("The source stage type must be the same as the destination stage type")
            }

            let removed_objects = source_stage_opt
                .map(|stage| stage.delete_many(&object_ids))
                .unwrap();

            self.update_frame_locations(&object_ids, dest_index);

            let mut payloads = Vec::with_capacity(removed_objects.len());
            for (id, payload) in removed_objects {
                let payload = match payload {
                    PipelinePayload::Frame(frame, updates, ctx) => {
                        ctx.span().end();
                        let ctx = self.get_stage_span(id, format!("stage/{}", dest_stage_name));
                        PipelinePayload::Frame(frame, updates, ctx)
                    }
                    PipelinePayload::Batch(batch, updates, contexts) => {
                        let mut new_contexts = HashMap::new();
                        for (id, ctx) in contexts.iter() {
                            ctx.span().end();
                            let ctx =
                                self.get_stage_span(*id, format!("stage/{}", dest_stage_name));
                            new_contexts.insert(*id, ctx);
                        }
                        PipelinePayload::Batch(batch, updates, new_contexts)
                    }
                };
                payloads.push((id, payload));
            }

            dest_stage.add_payloads(payloads)?;

            Ok(())
        }

        pub fn move_and_pack_frames(
            &self,
            dest_stage_name: &str,
            frame_ids: Vec<i64>,
        ) -> Result<i64> {
            let source_index = self.check_ids_in_the_same_stage(&frame_ids)?;
            let source_stage_opt = self.stages.get(source_index);
            if source_stage_opt.is_none() {
                bail!("Source stage not found")
            }
            let source_stage = source_stage_opt.unwrap();

            let dest_stage_opt = self.find_stage(dest_stage_name, source_index);
            if dest_stage_opt.is_none() {
                bail!("Destination stage not found")
            }

            let (dest_index, dest_stage) = dest_stage_opt.unwrap();

            if matches!(source_stage.stage_type, PipelineStagePayloadType::Batch)
                || matches!(dest_stage.stage_type, PipelineStagePayloadType::Frame)
            {
                bail!("Source stage must contain independent frames and destination stage must contain batched frames")
            }

            let batch_id = self.id_counter.fetch_add(1, Ordering::SeqCst) + 1;

            self.update_frame_locations(&frame_ids, dest_index);

            let mut batch = VideoFrameBatch::new();
            let mut batch_updates = Vec::new();
            let mut contexts = HashMap::new();

            for id in frame_ids {
                if let Some(payload) = source_stage_opt
                    .map(|source_stage| source_stage.delete(id))
                    .unwrap()
                {
                    match payload {
                        PipelinePayload::Frame(frame, updates, ctx) => {
                            batch.add(id, frame);
                            contexts.insert(id, ctx);
                            for update in updates {
                                batch_updates.push((id, update));
                            }
                        }
                        _ => bail!("Source stage must contain independent frames"),
                    }
                }
            }

            let contexts = contexts
                .into_iter()
                .map(|(id, ctx)| {
                    ctx.span().end();
                    let ctx = self.get_stage_span(id, format!("stage/{}", dest_stage_name));
                    (id, ctx)
                })
                .collect();

            let payload = PipelinePayload::Batch(batch, batch_updates, contexts);
            dest_stage.add_batch_payload(batch_id, payload)?;
            self.frame_locations.write().insert(batch_id, dest_index);

            Ok(batch_id)
        }

        pub fn move_and_unpack_batch(
            &self,
            dest_stage_name: &str,
            batch_id: i64,
        ) -> Result<Vec<i64>> {
            let source_index = self.get_stage_for_id(batch_id)?;
            let source_stage_opt = self.stages.get(source_index);
            if source_stage_opt.is_none() {
                bail!("Source stage not found")
            }
            let source_stage = source_stage_opt.unwrap();

            let dest_stage_opt = self.find_stage(dest_stage_name, source_index);
            if dest_stage_opt.is_none() {
                bail!("Destination stage not found")
            }

            let (dest_index, dest_stage) = dest_stage_opt.unwrap();

            if matches!(source_stage.stage_type, PipelineStagePayloadType::Frame)
                || matches!(dest_stage.stage_type, PipelineStagePayloadType::Batch)
            {
                bail!("Source stage must contain batched frames and destination stage must contain independent frames")
            }

            let (batch, updates, mut contexts) = if let Some(payload) = source_stage_opt
                .map(|stage| stage.delete(batch_id))
                .unwrap()
            {
                match payload {
                    PipelinePayload::Batch(batch, updates, contexts) => (batch, updates, contexts),
                    _ => bail!("Source stage must contain batch"),
                }
            } else {
                bail!("Batch not found in source stage")
            };

            self.frame_locations.write().remove(&batch_id);

            let frame_ids = batch.frames.keys().cloned().collect::<Vec<_>>();
            self.update_frame_locations(&frame_ids, dest_index);

            let mut payloads = HashMap::with_capacity(batch.frames.len());
            for (frame_id, frame) in batch.frames {
                let ctx = contexts.remove(&frame_id).unwrap();
                ctx.span().end();
                let ctx = self.get_stage_span(frame_id, format!("stage/{}", dest_stage_name));

                payloads.insert(frame_id, PipelinePayload::Frame(frame, Vec::new(), ctx));
            }

            for (frame_id, update) in updates {
                if let Some(frame) = payloads.get_mut(&frame_id) {
                    match frame {
                        PipelinePayload::Frame(_, updates, _) => {
                            updates.push(update);
                        }
                        _ => bail!("Destination stage must contain independent frames"),
                    }
                } else {
                    bail!("Frame not found in destination stage")
                }
            }

            dest_stage.add_payloads(payloads)?;

            Ok(frame_ids)
        }

        pub fn access_objects(
            &self,
            frame_id: i64,
            query: &MatchQuery,
        ) -> Result<HashMap<i64, Vec<VideoObjectProxy>>> {
            let stage = self.get_stage_for_id(frame_id)?;
            let stage_opt = self.stages.get(stage);
            if stage_opt.is_none() {
                bail!("Stage not found");
            }

            stage_opt
                .map(|stage| stage.access_objects(frame_id, query))
                .unwrap()
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::pipeline2::implementation::{Pipeline, PipelineStagePayloadType};
        use crate::primitives::attribute_value::{AttributeValue, AttributeValueVariant};
        use crate::primitives::frame_update::VideoFrameUpdate;
        use crate::primitives::{Attribute, AttributeMethods};
        use crate::test::gen_frame;
        use opentelemetry::global;
        use opentelemetry::sdk::export::trace::stdout;
        use opentelemetry::sdk::propagation::TraceContextPropagator;
        use opentelemetry::trace::{TraceContextExt, TraceId};
        use std::io::sink;
        use std::sync::atomic::Ordering;

        fn create_pipeline() -> anyhow::Result<Pipeline> {
            let pipeline = Pipeline::new(vec![
                ("input".to_string(), PipelineStagePayloadType::Frame),
                ("proc1".to_string(), PipelineStagePayloadType::Batch),
                ("proc2".to_string(), PipelineStagePayloadType::Batch),
                ("output".to_string(), PipelineStagePayloadType::Frame),
            ])?;
            Ok(pipeline)
        }

        #[test]
        fn test_new_pipeline() -> anyhow::Result<()> {
            let pipeline = create_pipeline()?;
            assert_eq!(pipeline.id_counter.load(Ordering::SeqCst), 0);
            assert_eq!(pipeline.stages.len(), 4);
            Ok(())
        }

        #[test]
        fn test_get_stage_type() -> anyhow::Result<()> {
            let pipeline = create_pipeline()?;
            assert!(matches!(
                pipeline.find_stage_type("input", 0),
                Some(PipelineStagePayloadType::Frame)
            ));
            assert!(matches!(
                pipeline.find_stage_type("proc1", 0),
                Some(PipelineStagePayloadType::Batch)
            ));
            Ok(())
        }

        #[test]
        fn test_add_del_frame() -> anyhow::Result<()> {
            let pipeline = create_pipeline()?;
            let id = pipeline.add_frame("input", gen_frame())?;
            assert_eq!(pipeline.get_stage_queue_len("input")?, 1);
            assert!(pipeline.add_frame("proc1", gen_frame()).is_err());
            assert_eq!(pipeline.get_stage_queue_len("proc1")?, 0);

            pipeline.delete(id)?;
            assert_eq!(pipeline.get_stage_queue_len("input")?, 0);

            Ok(())
        }

        #[test]
        fn test_frame_to_batch() -> anyhow::Result<()> {
            let pipeline = create_pipeline()?;
            let id = pipeline.add_frame("input", gen_frame())?;
            let batch_id = pipeline.move_and_pack_frames("proc1", vec![id])?;

            assert!(pipeline.get_independent_frame(id).is_err());
            assert_eq!(pipeline.get_stage_queue_len("input")?, 0);
            assert_eq!(pipeline.get_stage_queue_len("proc1")?, 1);
            pipeline.get_batch(batch_id)?;
            pipeline.get_batched_frame(batch_id, id)?;
            Ok(())
        }

        #[test]
        fn test_batch_to_frame() -> anyhow::Result<()> {
            let pipeline = create_pipeline()?;
            let id = pipeline.add_frame("input", gen_frame())?;
            let batch_id = pipeline.move_and_pack_frames("proc2", vec![id])?;
            assert_eq!(pipeline.get_stage_queue_len("input")?, 0);
            assert_eq!(pipeline.get_stage_queue_len("proc2")?, 1);
            assert_eq!(pipeline.get_stage_queue_len("output")?, 0);
            pipeline.move_and_unpack_batch("output", batch_id)?;
            assert_eq!(pipeline.get_stage_queue_len("input")?, 0);
            assert_eq!(pipeline.get_stage_queue_len("proc2")?, 0);
            assert_eq!(pipeline.get_stage_queue_len("output")?, 1);
            let _frame = pipeline.get_independent_frame(id)?;
            Ok(())
        }

        #[test]
        fn test_batch_to_batch() -> anyhow::Result<()> {
            let pipeline = create_pipeline()?;
            let id = pipeline.add_frame("input", gen_frame())?;
            let batch_id = pipeline.move_and_pack_frames("proc1", vec![id])?;
            pipeline.move_as_is("proc2", vec![batch_id])?;
            assert_eq!(pipeline.get_stage_queue_len("input")?, 0);
            assert_eq!(pipeline.get_stage_queue_len("proc1")?, 0);
            assert_eq!(pipeline.get_stage_queue_len("proc2")?, 1);
            let _batch = pipeline.get_batch(batch_id)?;
            let _frame = pipeline.get_batched_frame(batch_id, id)?;
            Ok(())
        }

        #[test]
        fn test_frame_to_frame() -> anyhow::Result<()> {
            let pipeline = create_pipeline()?;
            let id = pipeline.add_frame("input", gen_frame())?;
            pipeline.move_as_is("output", vec![id])?;
            assert_eq!(pipeline.get_stage_queue_len("input")?, 0);
            assert_eq!(pipeline.get_stage_queue_len("output")?, 1);
            let _frame = pipeline.get_independent_frame(id)?;
            Ok(())
        }

        fn get_update() -> VideoFrameUpdate {
            let mut update = VideoFrameUpdate::default();
            update.add_frame_attribute(Attribute::persistent(
                "update".into(),
                "attribute".into(),
                vec![AttributeValue::new(
                    AttributeValueVariant::String("1".into()),
                    None,
                )],
                Some("test".into()),
            ));
            update
        }

        #[test]
        fn test_frame_update() -> anyhow::Result<()> {
            let pipeline = create_pipeline()?;
            let id = pipeline.add_frame("input", gen_frame())?;
            let update = get_update();
            pipeline.add_frame_update(id, update)?;
            pipeline.apply_updates(id)?;
            let (frame, _) = pipeline.get_independent_frame(id)?;
            frame
                .get_attribute("update".to_string(), "attribute".to_string())
                .unwrap();
            Ok(())
        }

        #[test]
        fn test_batch_update() -> anyhow::Result<()> {
            let pipeline = create_pipeline()?;
            let id = pipeline.add_frame("input", gen_frame())?;
            let batch_id = pipeline.move_and_pack_frames("proc1", vec![id])?;
            let update = get_update();
            pipeline.add_batched_frame_update(batch_id, id, update)?;
            pipeline.apply_updates(batch_id)?;
            pipeline.clear_updates(batch_id)?;
            let (frame, _) = pipeline.get_batched_frame(batch_id, id)?;
            frame
                .get_attribute("update".to_string(), "attribute".to_string())
                .unwrap();
            Ok(())
        }

        #[test]
        fn test_sampling() -> anyhow::Result<()> {
            stdout::new_pipeline().with_writer(sink()).install_simple();
            global::set_text_map_propagator(TraceContextPropagator::new());

            let pipeline = create_pipeline()?;
            pipeline.set_sampling_period(2)?;

            let id = pipeline.add_frame("input", gen_frame())?;
            let (_frame, ctx) = pipeline.get_independent_frame(id)?;
            assert_eq!(ctx.span().span_context().trace_id(), TraceId::INVALID);

            let id = pipeline.add_frame("input", gen_frame())?;
            let (_frame, ctx) = pipeline.get_independent_frame(id)?;
            assert_ne!(ctx.span().span_context().trace_id(), TraceId::INVALID);

            let id = pipeline.add_frame("input", gen_frame())?;
            let (_frame, ctx) = pipeline.get_independent_frame(id)?;
            assert_eq!(ctx.span().span_context().trace_id(), TraceId::INVALID);

            let id = pipeline.add_frame("input", gen_frame())?;
            let (_frame, ctx) = pipeline.get_independent_frame(id)?;
            assert_ne!(ctx.span().span_context().trace_id(), TraceId::INVALID);

            Ok(())
        }

        #[test]
        fn test_no_tracing() -> anyhow::Result<()> {
            stdout::new_pipeline().with_writer(sink()).install_simple();
            global::set_text_map_propagator(TraceContextPropagator::new());

            let pipeline = create_pipeline()?;
            pipeline.set_sampling_period(0)?;

            let id = pipeline.add_frame("input", gen_frame())?;
            let (_frame, ctx) = pipeline.get_independent_frame(id)?;
            assert_eq!(ctx.span().span_context().trace_id(), TraceId::INVALID);

            let id = pipeline.add_frame("input", gen_frame())?;
            let (_frame, ctx) = pipeline.get_independent_frame(id)?;
            assert_eq!(ctx.span().span_context().trace_id(), TraceId::INVALID);

            Ok(())
        }

        #[test]
        fn test_tracing_every() -> anyhow::Result<()> {
            stdout::new_pipeline().with_writer(sink()).install_simple();
            global::set_text_map_propagator(TraceContextPropagator::new());

            let pipeline = create_pipeline()?;
            pipeline.set_sampling_period(1)?;

            let id = pipeline.add_frame("input", gen_frame())?;
            let (_frame, ctx) = pipeline.get_independent_frame(id)?;
            assert_ne!(ctx.span().span_context().trace_id(), TraceId::INVALID);

            let id = pipeline.add_frame("input", gen_frame())?;
            let (_frame, ctx) = pipeline.get_independent_frame(id)?;
            assert_ne!(ctx.span().span_context().trace_id(), TraceId::INVALID);

            Ok(())
        }
    }
}