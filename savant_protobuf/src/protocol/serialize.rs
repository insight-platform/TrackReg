use crate::protocol::generated;
use savant_core::message::MessageEnvelope;
use savant_core::primitives::attribute_value::{AttributeValue, AttributeValueVariant};
use savant_core::primitives::eos::EndOfStream;
use savant_core::primitives::frame::{
    VideoFrame, VideoFrameContent, VideoFrameProxy, VideoFrameTranscodingMethod,
    VideoFrameTransformation,
};
use savant_core::primitives::frame_batch::VideoFrameBatch;
use savant_core::primitives::frame_update::{
    AttributeUpdatePolicy, ObjectUpdatePolicy, VideoFrameUpdate,
};
use savant_core::primitives::object::VideoObjectProxy;
use savant_core::primitives::rust::UserData;
use savant_core::primitives::shutdown::Shutdown;
use savant_core::primitives::{
    Attribute, AttributeMethods, IntersectionKind, OwnedRBBoxData, PolygonalArea, RBBox,
};
use std::mem::transmute;
use std::sync::Arc;
use uuid::Uuid;

impl From<&VideoFrameProxy> for generated::VideoFrame {
    fn from(vfp: &VideoFrameProxy) -> Self {
        let bind = vfp.get_inner();
        let o = bind.read();
        generated::VideoFrame::from(&*o)
    }
}

impl From<&VideoFrameTranscodingMethod> for generated::VideoFrameTranscodingMethod {
    fn from(value: &VideoFrameTranscodingMethod) -> Self {
        match value {
            VideoFrameTranscodingMethod::Copy => generated::VideoFrameTranscodingMethod::Copy,
            VideoFrameTranscodingMethod::Encoded => generated::VideoFrameTranscodingMethod::Encoded,
        }
    }
}

impl From<generated::VideoFrameTranscodingMethod> for VideoFrameTranscodingMethod {
    fn from(value: generated::VideoFrameTranscodingMethod) -> Self {
        match value {
            generated::VideoFrameTranscodingMethod::Copy => VideoFrameTranscodingMethod::Copy,
            generated::VideoFrameTranscodingMethod::Encoded => VideoFrameTranscodingMethod::Encoded,
        }
    }
}

impl From<&VideoFrameContent> for generated::video_frame::Content {
    fn from(value: &VideoFrameContent) -> Self {
        match value {
            VideoFrameContent::External(e) => {
                generated::video_frame::Content::External(generated::ExternalFrame {
                    method: e.method.clone(),
                    location: e.location.clone(),
                })
            }
            VideoFrameContent::Internal(data) => {
                generated::video_frame::Content::Internal(data.clone())
            }
            VideoFrameContent::None => {
                generated::video_frame::Content::None(generated::NoneFrame {})
            }
        }
    }
}

impl From<generated::video_frame::Content> for VideoFrameContent {
    fn from(value: generated::video_frame::Content) -> Self {
        match value {
            generated::video_frame::Content::External(e) => {
                VideoFrameContent::External(savant_core::primitives::frame::ExternalFrame {
                    method: e.method,
                    location: e.location,
                })
            }
            generated::video_frame::Content::Internal(data) => VideoFrameContent::Internal(data),
            generated::video_frame::Content::None(_) => VideoFrameContent::None,
        }
    }
}

impl From<&VideoFrameTransformation> for generated::VideoFrameTransformation {
    fn from(value: &VideoFrameTransformation) -> Self {
        match value {
            VideoFrameTransformation::InitialSize(w, h) => generated::VideoFrameTransformation {
                transformation: Some(
                    generated::video_frame_transformation::Transformation::InitialSize(
                        generated::InitialSize {
                            width: *w,
                            height: *h,
                        },
                    ),
                ),
            },
            VideoFrameTransformation::Scale(w, h) => generated::VideoFrameTransformation {
                transformation: Some(
                    generated::video_frame_transformation::Transformation::Scale(
                        generated::Scale {
                            width: *w,
                            height: *h,
                        },
                    ),
                ),
            },
            VideoFrameTransformation::Padding(l, t, r, b) => generated::VideoFrameTransformation {
                transformation: Some(
                    generated::video_frame_transformation::Transformation::Padding(
                        generated::Padding {
                            padding_left: *l,
                            padding_top: *t,
                            padding_right: *r,
                            padding_bottom: *b,
                        },
                    ),
                ),
            },
            VideoFrameTransformation::ResultingSize(w, h) => generated::VideoFrameTransformation {
                transformation: Some(
                    generated::video_frame_transformation::Transformation::ResultingSize(
                        generated::ResultingSize {
                            width: *w,
                            height: *h,
                        },
                    ),
                ),
            },
        }
    }
}

impl From<&generated::VideoFrameTransformation> for VideoFrameTransformation {
    fn from(value: &generated::VideoFrameTransformation) -> Self {
        match &value.transformation {
            Some(generated::video_frame_transformation::Transformation::InitialSize(is)) => {
                VideoFrameTransformation::InitialSize(is.width, is.height)
            }
            Some(generated::video_frame_transformation::Transformation::Scale(s)) => {
                VideoFrameTransformation::Scale(s.width, s.height)
            }
            Some(generated::video_frame_transformation::Transformation::Padding(p)) => {
                VideoFrameTransformation::Padding(
                    p.padding_left,
                    p.padding_top,
                    p.padding_right,
                    p.padding_bottom,
                )
            }
            Some(generated::video_frame_transformation::Transformation::ResultingSize(rs)) => {
                VideoFrameTransformation::ResultingSize(rs.width, rs.height)
            }
            None => unreachable!("Transformation is not set"),
        }
    }
}

impl From<&Box<VideoFrame>> for generated::VideoFrame {
    fn from(vf: &Box<VideoFrame>) -> Self {
        generated::VideoFrame {
            previous_frame_seq_id: vf.previous_frame_seq_id,
            source_id: vf.source_id.clone(),
            uuid: Uuid::from_u128(vf.uuid).to_string(),
            creation_timestamp_ns_high: (vf.creation_timestamp_ns >> 64) as u64,
            creation_timestamp_ns_low: (vf.creation_timestamp_ns & 0xFFFFFFFFFFFFFFFF) as u64,
            framerate: vf.framerate.clone(),
            width: vf.width,
            height: vf.height,
            transcoding_method: generated::VideoFrameTranscodingMethod::from(&vf.transcoding_method)
                as i32,
            codec: vf.codec.clone(),
            keyframe: vf.keyframe.clone(),
            time_base_numerator: vf.time_base.0,
            time_base_denominator: vf.time_base.1,
            pts: vf.pts,
            dts: vf.dts.clone(),
            duration: vf.duration.clone(),
            attributes: vf.attributes.values().map(|a| a.into()).collect(),
            objects: vf
                .get_resident_objects()
                .values()
                .map(|o| generated::VideoObject::from(&VideoObjectProxy::from(o.clone())))
                .collect(),
            content: Some((&vf.content).into()),
            transformations: vf.transformations.iter().map(|t| t.into()).collect(),
        }
    }
}

impl From<&generated::VideoFrame> for VideoFrame {
    fn from(value: &generated::VideoFrame) -> Self {
        todo!()
    }
}

impl From<&VideoFrameBatch> for generated::VideoFrameBatch {
    fn from(b: &VideoFrameBatch) -> Self {
        generated::VideoFrameBatch {
            batch: b
                .frames()
                .iter()
                .map(|(id, f)| (*id, generated::VideoFrame::from(f)))
                .collect(),
        }
    }
}

impl From<&generated::VideoFrameBatch> for VideoFrameBatch {
    fn from(b: &generated::VideoFrameBatch) -> Self {
        let mut batch = VideoFrameBatch::new();
        for (id, f) in b.batch.iter() {
            batch.add(*id, VideoFrameProxy::from(f));
        }
        batch
    }
}

impl From<RBBox> for generated::BoundingBox {
    fn from(value: RBBox) -> Self {
        generated::BoundingBox {
            xc: value.get_xc(),
            yc: value.get_yc(),
            width: value.get_width(),
            height: value.get_height(),
            angle: value.get_angle(),
        }
    }
}

impl From<&generated::BoundingBox> for RBBox {
    fn from(value: &generated::BoundingBox) -> Self {
        RBBox::new(value.xc, value.yc, value.width, value.height, value.angle)
    }
}

impl From<&generated::VideoFrame> for VideoFrameProxy {
    fn from(value: &generated::VideoFrame) -> Self {
        todo!()
    }
}

impl From<&VideoObjectProxy> for generated::VideoObject {
    fn from(vop: &VideoObjectProxy) -> Self {
        generated::VideoObject {
            id: vop.get_id(),
            parent_id: vop.get_parent_id(),
            namespace: vop.get_namespace(),
            label: vop.get_label(),
            draw_label: vop.get_draw_label(),
            detection_box: Some(vop.get_detection_box().into()),
            attributes: vop
                .get_attributes()
                .iter()
                .map(|(ns, l)| {
                    generated::Attribute::from(&vop.get_attribute(ns.clone(), l.clone()).unwrap())
                })
                .collect(),
            confidence: vop.get_confidence(),
            track_box: vop.get_track_box().map(|rbbox| rbbox.into()),
            track_id: vop.get_track_id(),
        }
    }
}

impl From<&(VideoObjectProxy, Option<i64>)> for generated::VideoObjectWithForeignParent {
    fn from(p: &(VideoObjectProxy, Option<i64>)) -> Self {
        generated::VideoObjectWithForeignParent {
            object: Some(generated::VideoObject::from(&p.0)),
            parent_id: p.1.clone(),
        }
    }
}

impl From<&generated::VideoObjectWithForeignParent> for VideoObjectProxy {
    fn from(value: &generated::VideoObjectWithForeignParent) -> Self {
        todo!()
    }
}

impl From<AttributeUpdatePolicy> for generated::AttributeUpdatePolicy {
    fn from(p: AttributeUpdatePolicy) -> Self {
        match p {
            AttributeUpdatePolicy::ReplaceWithForeign => {
                generated::AttributeUpdatePolicy::ReplaceWithForeign
            }
            AttributeUpdatePolicy::KeepOwn => generated::AttributeUpdatePolicy::KeepOwn,
            AttributeUpdatePolicy::Error => generated::AttributeUpdatePolicy::Error,
        }
    }
}

impl From<&generated::AttributeUpdatePolicy> for AttributeUpdatePolicy {
    fn from(p: &generated::AttributeUpdatePolicy) -> Self {
        match p {
            generated::AttributeUpdatePolicy::ReplaceWithForeign => {
                AttributeUpdatePolicy::ReplaceWithForeign
            }
            generated::AttributeUpdatePolicy::KeepOwn => AttributeUpdatePolicy::KeepOwn,
            generated::AttributeUpdatePolicy::Error => AttributeUpdatePolicy::Error,
        }
    }
}

impl From<ObjectUpdatePolicy> for generated::ObjectUpdatePolicy {
    fn from(p: ObjectUpdatePolicy) -> Self {
        match p {
            ObjectUpdatePolicy::AddForeignObjects => {
                generated::ObjectUpdatePolicy::AddForeignObjects
            }
            ObjectUpdatePolicy::ErrorIfLabelsCollide => {
                generated::ObjectUpdatePolicy::ErrorIfLabelsCollide
            }
            ObjectUpdatePolicy::ReplaceSameLabelObjects => {
                generated::ObjectUpdatePolicy::ReplaceSameLabelObjects
            }
        }
    }
}

impl From<&generated::ObjectUpdatePolicy> for ObjectUpdatePolicy {
    fn from(p: &generated::ObjectUpdatePolicy) -> Self {
        match p {
            generated::ObjectUpdatePolicy::AddForeignObjects => {
                ObjectUpdatePolicy::AddForeignObjects
            }
            generated::ObjectUpdatePolicy::ErrorIfLabelsCollide => {
                ObjectUpdatePolicy::ErrorIfLabelsCollide
            }
            generated::ObjectUpdatePolicy::ReplaceSameLabelObjects => {
                ObjectUpdatePolicy::ReplaceSameLabelObjects
            }
        }
    }
}

impl From<&VideoFrameUpdate> for generated::VideoFrameUpdate {
    fn from(vfu: &VideoFrameUpdate) -> Self {
        generated::VideoFrameUpdate {
            frame_attributes: vfu
                .get_frame_attributes()
                .iter()
                .map(|a| a.into())
                .collect(),
            object_attributes: vfu
                .get_object_attributes()
                .iter()
                .map(|oa| generated::ObjectAttribute {
                    object_id: oa.0,
                    attribute: Some(generated::Attribute::from(&oa.1)),
                })
                .collect(),
            objects: vfu.get_objects().iter().map(|o| o.into()).collect(),
            frame_attribute_policy: generated::AttributeUpdatePolicy::from(
                vfu.get_frame_attribute_policy(),
            ) as i32,
            object_attribute_policy: generated::AttributeUpdatePolicy::from(
                vfu.get_object_attribute_policy(),
            ) as i32,
            object_policy: generated::ObjectUpdatePolicy::from(vfu.get_object_policy()) as i32,
        }
    }
}

impl From<&generated::VideoFrameUpdate> for VideoFrameUpdate {
    fn from(value: &generated::VideoFrameUpdate) -> Self {
        todo!()
    }
}

impl From<&PolygonalArea> for generated::PolygonalArea {
    fn from(poly: &PolygonalArea) -> Self {
        generated::PolygonalArea {
            points: poly
                .get_vertices()
                .iter()
                .map(|p| generated::Point { x: p.x, y: p.y })
                .collect(),
            tags: poly.get_tags().map(|tags| generated::PolygonalAreaTags {
                tags: tags
                    .iter()
                    .map(|t| generated::PolygonalAreaTag { tag: t.clone() })
                    .collect(),
            }),
        }
    }
}

impl From<&generated::PolygonalArea> for PolygonalArea {
    fn from(value: &generated::PolygonalArea) -> Self {
        todo!()
    }
}

impl From<&IntersectionKind> for generated::IntersectionKind {
    fn from(kind: &IntersectionKind) -> Self {
        match kind {
            IntersectionKind::Inside => generated::IntersectionKind::Inside,
            IntersectionKind::Outside => generated::IntersectionKind::Outside,
            IntersectionKind::Enter => generated::IntersectionKind::Enter,
            IntersectionKind::Leave => generated::IntersectionKind::Leave,
            IntersectionKind::Cross => generated::IntersectionKind::Cross,
        }
    }
}

impl From<&generated::IntersectionKind> for IntersectionKind {
    fn from(kind: &generated::IntersectionKind) -> Self {
        match kind {
            generated::IntersectionKind::Inside => IntersectionKind::Inside,
            generated::IntersectionKind::Outside => IntersectionKind::Outside,
            generated::IntersectionKind::Enter => IntersectionKind::Enter,
            generated::IntersectionKind::Leave => IntersectionKind::Leave,
            generated::IntersectionKind::Cross => IntersectionKind::Cross,
        }
    }
}

impl From<&OwnedRBBoxData> for generated::BoundingBox {
    fn from(value: &OwnedRBBoxData) -> Self {
        generated::BoundingBox {
            xc: value.xc,
            yc: value.yc,
            width: value.width,
            height: value.height,
            angle: value.angle.clone(),
        }
    }
}

impl From<&generated::BoundingBox> for OwnedRBBoxData {
    fn from(value: &generated::BoundingBox) -> Self {
        OwnedRBBoxData {
            xc: value.xc,
            yc: value.yc,
            width: value.width,
            height: value.height,
            angle: value.angle.clone(),
            has_modifications: false,
        }
    }
}

impl From<&AttributeValueVariant> for generated::attribute_value::Value {
    fn from(value: &AttributeValueVariant) -> Self {
        match value {
            AttributeValueVariant::Bytes(dims, data) => {
                generated::attribute_value::Value::Bytes(generated::BytesAttributeValueVariant {
                    dims: dims.clone(),
                    data: data.clone(),
                })
            }
            AttributeValueVariant::String(s) => {
                generated::attribute_value::Value::String(generated::StringAttributeValueVariant {
                    data: s.clone(),
                })
            }
            AttributeValueVariant::StringVector(sv) => {
                generated::attribute_value::Value::StringVector(
                    generated::StringVectorAttributeValueVariant { data: sv.clone() },
                )
            }
            AttributeValueVariant::Integer(i) => generated::attribute_value::Value::Integer(
                generated::IntegerAttributeValueVariant { data: *i },
            ),
            AttributeValueVariant::IntegerVector(iv) => {
                generated::attribute_value::Value::IntegerVector(
                    generated::IntegerVectorAttributeValueVariant { data: iv.clone() },
                )
            }
            AttributeValueVariant::Float(f) => {
                generated::attribute_value::Value::Float(generated::FloatAttributeValueVariant {
                    data: *f,
                })
            }
            AttributeValueVariant::FloatVector(fv) => {
                generated::attribute_value::Value::FloatVector(
                    generated::FloatVectorAttributeValueVariant { data: fv.clone() },
                )
            }
            AttributeValueVariant::Boolean(b) => generated::attribute_value::Value::Boolean(
                generated::BooleanAttributeValueVariant { data: *b },
            ),
            AttributeValueVariant::BooleanVector(bv) => {
                generated::attribute_value::Value::BooleanVector(
                    generated::BooleanVectorAttributeValueVariant { data: bv.clone() },
                )
            }
            AttributeValueVariant::BBox(bb) => generated::attribute_value::Value::BoundingBox(
                generated::BoundingBoxAttributeValueVariant {
                    data: Some(generated::BoundingBox::from(bb)),
                },
            ),
            AttributeValueVariant::BBoxVector(bbv) => {
                generated::attribute_value::Value::BoundingBoxVector(
                    generated::BoundingBoxVectorAttributeValueVariant {
                        data: bbv
                            .iter()
                            .map(|bb| generated::BoundingBox::from(bb))
                            .collect(),
                    },
                )
            }
            AttributeValueVariant::Point(p) => {
                generated::attribute_value::Value::Point(generated::PointAttributeValueVariant {
                    data: Some(generated::Point { x: p.x, y: p.y }),
                })
            }
            AttributeValueVariant::PointVector(pv) => {
                generated::attribute_value::Value::PointVector(
                    generated::PointVectorAttributeValueVariant {
                        data: pv
                            .iter()
                            .map(|p| generated::Point { x: p.x, y: p.y })
                            .collect(),
                    },
                )
            }
            AttributeValueVariant::Polygon(poly) => generated::attribute_value::Value::Polygon(
                generated::PolygonAttributeValueVariant {
                    data: Some(poly.into()),
                },
            ),
            AttributeValueVariant::PolygonVector(pv) => {
                generated::attribute_value::Value::PolygonVector(
                    generated::PolygonVectorAttributeValueVariant {
                        data: pv.iter().map(|poly| poly.into()).collect(),
                    },
                )
            }
            AttributeValueVariant::Intersection(is) => {
                generated::attribute_value::Value::Intersection(
                    generated::IntersectionAttributeValueVariant {
                        data: Some(generated::Intersection {
                            kind: generated::IntersectionKind::from(&is.kind) as i32,
                            edges: is
                                .edges
                                .iter()
                                .map(|e| generated::IntersectionEdge {
                                    id: e.0 as u64,
                                    tag: e.1.clone(),
                                })
                                .collect(),
                        }),
                    },
                )
            }
            AttributeValueVariant::TemporaryValue(_) => {
                unreachable!("TemporaryValue is not supported")
            }
            AttributeValueVariant::None => {
                generated::attribute_value::Value::None(generated::NoneAttributeValueVariant {})
            }
        }
    }
}

impl From<&generated::attribute_value::Value> for AttributeValueVariant {
    fn from(value: &generated::attribute_value::Value) -> Self {
        match value {
            generated::attribute_value::Value::Bytes(b) => {
                AttributeValueVariant::Bytes(b.dims.clone(), b.data.clone())
            }
            generated::attribute_value::Value::String(s) => {
                AttributeValueVariant::String(s.data.clone())
            }
            generated::attribute_value::Value::StringVector(sv) => {
                AttributeValueVariant::StringVector(sv.data.clone())
            }
            generated::attribute_value::Value::Integer(i) => {
                AttributeValueVariant::Integer(i.data.clone())
            }
            generated::attribute_value::Value::IntegerVector(iv) => {
                AttributeValueVariant::IntegerVector(iv.data.clone())
            }
            generated::attribute_value::Value::Float(f) => {
                AttributeValueVariant::Float(f.data.clone())
            }
            generated::attribute_value::Value::FloatVector(fv) => {
                AttributeValueVariant::FloatVector(fv.data.clone())
            }
            generated::attribute_value::Value::Boolean(b) => {
                AttributeValueVariant::Boolean(b.data.clone())
            }
            generated::attribute_value::Value::BooleanVector(bv) => {
                AttributeValueVariant::BooleanVector(bv.data.clone())
            }
            generated::attribute_value::Value::BoundingBox(bb) => {
                AttributeValueVariant::BBox(bb.data.as_ref().unwrap().into())
            }
            generated::attribute_value::Value::BoundingBoxVector(bbv) => {
                AttributeValueVariant::BBoxVector(bbv.data.iter().map(|bb| bb.into()).collect())
            }
            generated::attribute_value::Value::Point(p) => {
                AttributeValueVariant::Point(savant_core::primitives::Point::new(
                    p.data.as_ref().unwrap().x,
                    p.data.as_ref().unwrap().y,
                ))
            }
            generated::attribute_value::Value::PointVector(pv) => {
                AttributeValueVariant::PointVector(
                    pv.data
                        .iter()
                        .map(|p| savant_core::primitives::Point::new(p.x, p.y))
                        .collect(),
                )
            }
            generated::attribute_value::Value::Polygon(poly) => {
                AttributeValueVariant::Polygon(poly.data.as_ref().unwrap().into())
            }
            generated::attribute_value::Value::PolygonVector(pv) => {
                AttributeValueVariant::PolygonVector(
                    pv.data.iter().map(|poly| poly.into()).collect(),
                )
            }
            generated::attribute_value::Value::Intersection(i) => {
                AttributeValueVariant::Intersection(savant_core::primitives::Intersection {
                    kind: IntersectionKind::from(&unsafe {
                        transmute::<i32, generated::IntersectionKind>(i.data.as_ref().unwrap().kind)
                    }),

                    edges: i
                        .data
                        .as_ref()
                        .unwrap()
                        .edges
                        .iter()
                        .map(|e| (e.id as usize, e.tag.clone()))
                        .collect(),
                })
            }
            generated::attribute_value::Value::None(_) => AttributeValueVariant::None,
        }
    }
}

impl From<&AttributeValue> for generated::AttributeValue {
    fn from(value: &AttributeValue) -> Self {
        generated::AttributeValue {
            confidence: value.confidence,
            value: Some(generated::attribute_value::Value::from(&value.value)),
        }
    }
}

impl From<&generated::AttributeValue> for AttributeValue {
    fn from(value: &generated::AttributeValue) -> Self {
        AttributeValue {
            confidence: value.confidence.clone(),
            value: AttributeValueVariant::from(value.value.as_ref().unwrap()),
        }
    }
}

impl From<&Attribute> for generated::Attribute {
    fn from(a: &Attribute) -> Self {
        generated::Attribute {
            namespace: a.namespace.clone(),
            name: a.name.clone(),
            values: a.values.iter().map(|v| v.into()).collect(),
            hint: a.hint.clone(),
            is_persistent: a.is_persistent.clone(),
            is_hidden: a.is_hidden.clone(),
        }
    }
}

impl From<&generated::Attribute> for Attribute {
    fn from(value: &generated::Attribute) -> Self {
        Attribute {
            namespace: value.namespace.clone(),
            name: value.name.clone(),
            values: Arc::new(value.values.iter().map(|v| v.into()).collect()),
            hint: value.hint.clone(),
            is_persistent: value.is_persistent.clone(),
            is_hidden: value.is_hidden.clone(),
        }
    }
}

impl From<&UserData> for generated::UserData {
    fn from(ud: &UserData) -> Self {
        generated::UserData {
            source_id: ud.source_id.clone(),
            attributes: ud.attributes.values().map(|a| a.into()).collect(),
        }
    }
}

impl From<&generated::UserData> for UserData {
    fn from(value: &generated::UserData) -> Self {
        UserData {
            source_id: value.source_id.clone(),
            attributes: value
                .attributes
                .iter()
                .map(|a| ((a.namespace.clone(), a.name.clone()), a.into()))
                .collect(),
        }
    }
}

impl From<&MessageEnvelope> for generated::message::Content {
    fn from(value: &MessageEnvelope) -> Self {
        match value {
            MessageEnvelope::EndOfStream(eos) => {
                generated::message::Content::EndOfStream(generated::EndOfStream {
                    source_id: eos.source_id.clone(),
                })
            }
            MessageEnvelope::VideoFrame(vf) => generated::message::Content::VideoFrame(vf.into()),
            MessageEnvelope::VideoFrameBatch(vfb) => {
                generated::message::Content::VideoFrameBatch(vfb.into())
            }

            MessageEnvelope::VideoFrameUpdate(vfu) => {
                generated::message::Content::VideoFrameUpdate(vfu.into())
            }
            MessageEnvelope::UserData(ud) => generated::message::Content::UserData(ud.into()),
            MessageEnvelope::Shutdown(s) => {
                generated::message::Content::Shutdown(generated::Shutdown {
                    auth: s.auth.clone(),
                })
            }
            MessageEnvelope::Unknown(m) => {
                generated::message::Content::Unknown(generated::Unknown { message: m.clone() })
            }
        }
    }
}

impl From<&generated::message::Content> for MessageEnvelope {
    fn from(value: &generated::message::Content) -> Self {
        match value {
            generated::message::Content::EndOfStream(eos) => {
                MessageEnvelope::EndOfStream(EndOfStream {
                    source_id: eos.source_id.clone(),
                })
            }
            generated::message::Content::VideoFrame(vf) => {
                MessageEnvelope::VideoFrame(Box::new(VideoFrame::from(vf)))
            }
            generated::message::Content::VideoFrameBatch(vfb) => {
                MessageEnvelope::VideoFrameBatch(VideoFrameBatch::from(vfb))
            }
            generated::message::Content::VideoFrameUpdate(vfu) => {
                MessageEnvelope::VideoFrameUpdate(VideoFrameUpdate::from(vfu))
            }
            generated::message::Content::UserData(ud) => {
                MessageEnvelope::UserData(UserData::from(ud))
            }
            generated::message::Content::Shutdown(s) => MessageEnvelope::Shutdown(Shutdown {
                auth: s.auth.clone(),
            }),
            generated::message::Content::Unknown(u) => MessageEnvelope::Unknown(u.message.clone()),
        }
    }
}
