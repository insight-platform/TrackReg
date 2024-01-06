use crate::primitives::any_object::AnyObject;
use crate::primitives::attribute_value::{AttributeValue, AttributeValueVariant};
use crate::primitives::frame::{
    VideoFrameBuilder, VideoFrameContent, VideoFrameProxy, VideoFrameTranscodingMethod,
};
use crate::primitives::object::{
    IdCollisionResolutionPolicy, VideoObject, VideoObjectBuilder, VideoObjectProxy,
};
use crate::primitives::{Attribute, AttributeMethods, RBBox};
use std::sync::Arc;

type Variant = AttributeValueVariant;

pub fn gen_empty_frame() -> VideoFrameProxy {
    VideoFrameProxy::from_inner(
        VideoFrameBuilder::default()
            .source_id("test".to_string())
            .pts(0)
            .framerate("test".to_string())
            .width(0)
            .uuid(uuid::Uuid::new_v4().as_u128())
            .height(0)
            .content(Arc::new(VideoFrameContent::None))
            .transcoding_method(VideoFrameTranscodingMethod::Copy)
            .codec(None)
            .keyframe(None)
            .build()
            .unwrap(),
    )
}

pub fn gen_frame() -> VideoFrameProxy {
    let f = VideoFrameProxy::from_inner(
        VideoFrameBuilder::default()
            .source_id("test".to_string())
            .pts(1000000)
            .framerate("test".to_string())
            .width(1280)
            .uuid(uuid::Uuid::new_v4().as_u128())
            .height(720)
            .content(Arc::new(VideoFrameContent::None))
            .transcoding_method(VideoFrameTranscodingMethod::Copy)
            .codec(None)
            .keyframe(None)
            .build()
            .unwrap(),
    );

    let parent_object = VideoObjectProxy::from(
        VideoObjectBuilder::default()
            .id(0)
            .detection_box(RBBox::new(0.0, 0.0, 0.0, 0.0, None))
            .attributes(Vec::default())
            .confidence(None)
            .namespace("test".to_string())
            .label("test2".to_string())
            .build()
            .unwrap(),
    );

    let c1 = VideoObjectProxy::from(
        VideoObjectBuilder::default()
            .id(1)
            .detection_box(RBBox::new(0.0, 0.0, 0.0, 0.0, None))
            .parent_id(Some(parent_object.get_id()))
            .attributes(Vec::default())
            .confidence(None)
            .namespace("test2".to_string())
            .label("test".to_string())
            .build()
            .unwrap(),
    );

    let c2 = VideoObjectProxy::from(
        VideoObjectBuilder::default()
            .id(2)
            .detection_box(RBBox::new(0.0, 0.0, 0.0, 0.0, None))
            .parent_id(Some(parent_object.get_id()))
            .attributes(Vec::default())
            .confidence(None)
            .namespace("test2".to_string())
            .label("test2".to_string())
            .build()
            .unwrap(),
    );

    f.add_object(&parent_object, IdCollisionResolutionPolicy::Error)
        .unwrap();
    f.add_object(&c1, IdCollisionResolutionPolicy::Error)
        .unwrap();
    f.add_object(&c2, IdCollisionResolutionPolicy::Error)
        .unwrap();

    f.set_attribute(Attribute::persistent(
        "system",
        "test",
        vec![AttributeValue::string("1", None)],
        &Some("test"),
        false,
    ));

    f.set_attribute(Attribute::persistent(
        "system2",
        "test2",
        vec![AttributeValue::string("2", None)],
        &None,
        false,
    ));

    f.set_attribute(Attribute::persistent(
        "system",
        "test2",
        vec![AttributeValue::string("3", None)],
        &Some("test"),
        false,
    ));

    f.set_attribute(Attribute::persistent(
        "test",
        "test",
        vec![
            AttributeValue::bytes(&[8, 3, 8, 8], &[0; 192], None),
            AttributeValue::new(Variant::IntegerVector([0, 1, 2, 3, 4, 5].into()), None),
            AttributeValue::new(Variant::String("incoming".to_string()), Some(0.56)),
            AttributeValue::new(Variant::TemporaryValue(AnyObject::new(Box::new(1.0))), None),
        ],
        &Some("hint"),
        false,
    ));
    f
}

pub fn gen_object(id: i64) -> VideoObjectProxy {
    let o = VideoObjectProxy::from(VideoObject {
        id,
        namespace: s("peoplenet"),
        label: s("face"),
        confidence: Some(0.5),
        detection_box: RBBox::new(1.0, 2.0, 10.0, 20.0, None),
        track_id: Some(id),
        track_box: Some(RBBox::new(100.0, 200.0, 10.0, 20.0, None)),
        ..Default::default()
    });

    let attr = Attribute::persistent("some", "attribute", vec![], &Some("hint"), false);
    o.set_attribute(attr);
    o
}

#[inline(always)]
pub fn s(a: &str) -> String {
    a.to_string()
}
