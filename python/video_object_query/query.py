from savant_rs.primitives.geometry import RBBox
from savant_rs.utils import BBoxMetricType
from savant_rs.video_object_query import FloatExpression as FE, \
    IntExpression as IE, \
    StringExpression as SE, \
    MatchQuery as Q, \
    utility_resolver_name

from savant_rs.logging import LogLevel, set_log_level
set_log_level(LogLevel.Trace)

and_ = Q.and_
or_ = Q.or_
not_ = Q.not_


gt = IE.gt
lt = IE.lt
eq = IE.eq
fgt = FE.gt

q = and_(
    Q.eval("""!is_empty(id) || id == 13 || label == "hello" || namespace == "where" """),
    Q.namespace(SE.one_of('savant', 'deepstream')),
    Q.label(SE.one_of('person', 'cyclist')),
    Q.box_metric(RBBox(100.0, 50.0, 20.0, 30.0, 50), BBoxMetricType.IoU, FE.gt(0.5)),
    and_(
        or_(
            not_(Q.parent_defined()),
            or_(
                Q.parent_id(IE.one_of(0, 1, 2)),
                Q.parent_id(gt(10))
            )
        )
    ),
    Q.attributes_jmes_query("[?(name=='test' && namespace=='test')]"),
    Q.confidence(FE.gt(0.5)),
    Q.box_height(FE.gt(100)),
)

print("------------------------")
print("Condensed JSON:")
print("------------------------")
print(q.json)

print("------------------------")
print("Pretty JSON:")
print("------------------------")
print(q.json_pretty)

print("------------------------")
print("YAML:")
print("------------------------")
print(q.yaml)

q2 = Q.from_json(q.json)
assert q.json == q2.json

q3 = Q.from_yaml(q.yaml)
assert q3.yaml == q.yaml
