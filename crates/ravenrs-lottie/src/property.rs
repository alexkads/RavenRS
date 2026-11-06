//! Values that change over time: a Lottie property and its keyframes.

use serde_json::Value;

/// How finely a curved motion path is measured.
const ARC_SAMPLES: usize = 64;

/// A cubic timing curve from (0, 0) to (1, 1), as CSS has them.
fn timing(p1: (f32, f32), p2: (f32, f32), x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bezier = |a: f32, b: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
    };
    let slope = |a: f32, b: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
    };
    let mut t = x;
    for _ in 0..8 {
        let error = bezier(p1.0, p2.0, t) - x;
        if error.abs() < 1e-5 {
            break;
        }
        let d = slope(p1.0, p2.0, t);
        if d.abs() < 1e-6 {
            break;
        }
        t = (t - error / d).clamp(0.0, 1.0);
    }
    // Newton can stall on steep curves: halving always lands.
    if (bezier(p1.0, p2.0, t) - x).abs() > 1e-3 {
        let (mut low, mut high) = (0.0, 1.0);
        for _ in 0..24 {
            t = (low + high) / 2.0;
            if bezier(p1.0, p2.0, t) < x {
                low = t;
            } else {
                high = t;
            }
        }
    }
    bezier(p1.1, p2.1, t)
}

pub(crate) fn numbers(value: &Value) -> Vec<f32> {
    match value {
        Value::Number(n) => vec![n.as_f64().unwrap_or(0.0) as f32],
        Value::Array(list) => list.iter().flat_map(numbers).collect(),
        _ => Vec::new(),
    }
}

/// One dimension's or all dimensions' handle of a keyframe.
fn handle(value: Option<&Value>, dimension: usize, default: f32) -> f32 {
    let Some(value) = value else { return default };
    match value {
        Value::Array(list) => {
            list.get(dimension).or_else(|| list.first()).and_then(Value::as_f64).map_or(default, |v| v as f32)
        }
        Value::Number(n) => n.as_f64().map_or(default, |v| v as f32),
        _ => default,
    }
}

/// The easing of one dimension between two keyframes.
#[derive(Clone, Copy, Debug)]
struct Easing {
    out: (f32, f32),
    into: (f32, f32),
}

#[derive(Clone, Debug)]
pub(crate) struct Key<T> {
    frame: f32,
    value: T,
    /// Holds until the next keyframe.
    hold: bool,
    easing: Vec<Easing>,
    /// A position moves along a curve between these handles.
    tangent_out: Option<Vec<f32>>,
    tangent_in: Option<Vec<f32>>,
}

/// A property with numbers for a value.
#[derive(Clone, Debug)]
pub(crate) enum Numbers {
    Fixed(Vec<f32>),
    Keys(Vec<Key<Vec<f32>>>),
}

fn parse_keys<T: Clone>(list: &[Value], value_of: impl Fn(&Value) -> Option<T>) -> Vec<Key<T>> {
    let mut keys: Vec<Key<T>> = Vec::with_capacity(list.len());
    for (i, key) in list.iter().enumerate() {
        let frame = key.get("t").and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let start = key.get("s").and_then(&value_of);
        // Old files keep the end of each stretch in `e` and leave the last key empty.
        let value = match start {
            Some(value) => value,
            None => {
                let before = i.checked_sub(1).and_then(|p| list.get(p));
                match before.and_then(|k| k.get("e")).and_then(&value_of) {
                    Some(value) => value,
                    None => match keys.last() {
                        Some(last) => last.value.clone(),
                        None => continue,
                    },
                }
            }
        };
        let (o, i_handle) = (key.get("o"), key.get("i"));
        let dimensions = [o, i_handle]
            .iter()
            .filter_map(|h| h.and_then(|h| h.get("x")).and_then(Value::as_array).map(Vec::len))
            .max()
            .unwrap_or(1);
        let easing = (0..dimensions)
            .map(|d| Easing {
                out: (handle(o.and_then(|h| h.get("x")), d, 0.0), handle(o.and_then(|h| h.get("y")), d, 0.0)),
                into: (
                    handle(i_handle.and_then(|h| h.get("x")), d, 1.0),
                    handle(i_handle.and_then(|h| h.get("y")), d, 1.0),
                ),
            })
            .collect();
        keys.push(Key {
            frame,
            value,
            hold: key.get("h").and_then(Value::as_i64) == Some(1),
            easing,
            tangent_out: key.get("to").map(numbers).filter(|v| !v.is_empty()),
            tangent_in: key.get("ti").map(numbers).filter(|v| !v.is_empty()),
        });
    }
    keys
}

/// Which two keys `frame` falls between, and how far along.
fn locate<T>(keys: &[Key<T>], frame: f32) -> Option<(&Key<T>, Option<&Key<T>>)> {
    let first = keys.first()?;
    if frame <= first.frame || keys.len() == 1 {
        return Some((first, None));
    }
    for pair in keys.windows(2) {
        if frame < pair[1].frame {
            return Some((&pair[0], Some(&pair[1])));
        }
    }
    keys.last().map(|last| (last, None))
}

impl Numbers {
    pub(crate) fn parse(property: Option<&Value>) -> Self {
        let Some(property) = property else { return Self::Fixed(Vec::new()) };
        let Some(k) = property.get("k") else {
            return Self::Fixed(numbers(property));
        };
        if let Some(list) = k.as_array()
            && list.first().is_some_and(|first| first.get("t").is_some())
        {
            let keys = parse_keys(list, |v| Some(numbers(v)).filter(|n| !n.is_empty()));
            if !keys.is_empty() {
                return Self::Keys(keys);
            }
        }
        Self::Fixed(numbers(k))
    }

    pub(crate) fn at(&self, frame: f32) -> Vec<f32> {
        match self {
            Self::Fixed(value) => value.clone(),
            Self::Keys(keys) => {
                let Some((a, next)) = locate(keys, frame) else { return Vec::new() };
                let Some(b) = next.filter(|_| !a.hold) else { return a.value.clone() };
                let span = (b.frame - a.frame).max(1e-6);
                let x = (frame - a.frame) / span;
                // A curve through space, eased as a whole.
                if let (Some(to), Some(ti)) = (
                    &a.tangent_out,
                    &a.tangent_in.as_ref().or(b.tangent_in.as_ref()).or(a.tangent_in.as_ref()),
                ) && a.value.len() >= 2
                    && b.value.len() >= 2
                    && to.len() >= 2
                    && ti.len() >= 2
                    && let Some(e) = a.easing.first()
                {
                    let progress = timing(e.out, e.into, x);
                    let dimensions = a.value.len().min(b.value.len());
                    let point = |t: f32| -> Vec<f32> {
                        let u = 1.0 - t;
                        (0..dimensions)
                            .map(|d| {
                                let (p0, p3) = (a.value[d], b.value[d]);
                                let p1 = p0 + to.get(d).copied().unwrap_or(0.0);
                                let p2 = p3 + ti.get(d).copied().unwrap_or(0.0);
                                u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3
                            })
                            .collect()
                    };
                    // The easing runs along the length of the curve, not its parameter.
                    let samples: Vec<Vec<f32>> =
                        (0..=ARC_SAMPLES).map(|i| point(i as f32 / ARC_SAMPLES as f32)).collect();
                    let lengths: Vec<f32> = samples
                        .windows(2)
                        .map(|p| p[0].iter().zip(&p[1]).map(|(q, r)| (q - r).powi(2)).sum::<f32>().sqrt())
                        .collect();
                    let total: f32 = lengths.iter().sum();
                    if total <= 0.0 {
                        return point(progress);
                    }
                    let mut left = progress.clamp(0.0, 1.0) * total;
                    for (i, length) in lengths.iter().enumerate() {
                        if left <= *length {
                            let k = if *length > 0.0 { left / length } else { 0.0 };
                            return point((i as f32 + k) / ARC_SAMPLES as f32);
                        }
                        left -= length;
                    }
                    return point(1.0);
                }
                (0..a.value.len().min(b.value.len()))
                    .map(|d| {
                        let e = a.easing.get(d).or_else(|| a.easing.first());
                        let t = e.map_or(x, |e| timing(e.out, e.into, x));
                        a.value[d] + (b.value[d] - a.value[d]) * t
                    })
                    .collect()
            }
        }
    }

    pub(crate) fn first_at(&self, frame: f32, default: f32) -> f32 {
        self.at(frame).first().copied().unwrap_or(default)
    }

    pub(crate) fn pair_at(&self, frame: f32, default: (f32, f32)) -> (f32, f32) {
        let value = self.at(frame);
        (value.first().copied().unwrap_or(default.0), value.get(1).copied().unwrap_or(default.1))
    }
}

/// The points of a path at a keyframe: vertices with their handles.
#[derive(Clone, Debug, Default)]
pub(crate) struct Bezier {
    pub closed: bool,
    pub vertices: Vec<(f32, f32)>,
    pub tangents_in: Vec<(f32, f32)>,
    pub tangents_out: Vec<(f32, f32)>,
}

fn pairs(value: Option<&Value>) -> Vec<(f32, f32)> {
    value
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|pair| {
                    let n = numbers(pair);
                    (n.first().copied().unwrap_or(0.0), n.get(1).copied().unwrap_or(0.0))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_bezier(value: &Value) -> Option<Bezier> {
    // Keyframed shapes wrap each value in a list of one.
    let value = value.as_array().and_then(|list| list.first()).unwrap_or(value);
    let vertices = pairs(value.get("v"));
    if vertices.is_empty() {
        return None;
    }
    Some(Bezier {
        closed: value.get("c").and_then(Value::as_bool).unwrap_or(false),
        tangents_in: pairs(value.get("i")),
        tangents_out: pairs(value.get("o")),
        vertices,
    })
}

/// A property with a path for a value.
#[derive(Clone, Debug)]
pub(crate) enum Shape {
    Fixed(Bezier),
    Keys(Vec<Key<Bezier>>),
}

impl Shape {
    pub(crate) fn parse(property: Option<&Value>) -> Self {
        let k = property.and_then(|p| p.get("k"));
        if let Some(list) = k.and_then(Value::as_array)
            && list.first().is_some_and(|first| first.get("t").is_some())
        {
            let keys = parse_keys(list, parse_bezier);
            if !keys.is_empty() {
                return Self::Keys(keys);
            }
        }
        Self::Fixed(k.and_then(parse_bezier).unwrap_or_default())
    }

    pub(crate) fn at(&self, frame: f32) -> Bezier {
        match self {
            Self::Fixed(path) => path.clone(),
            Self::Keys(keys) => {
                let Some((a, next)) = locate(keys, frame) else { return Bezier::default() };
                let Some(b) = next.filter(|_| !a.hold) else { return a.value.clone() };
                let x = (frame - a.frame) / (b.frame - a.frame).max(1e-6);
                let t = a.easing.first().map_or(x, |e| timing(e.out, e.into, x));
                let mix = |from: &[(f32, f32)], to: &[(f32, f32)]| -> Vec<(f32, f32)> {
                    from.iter().zip(to).map(|(p, q)| (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t)).collect()
                };
                // Paths of different lengths cannot blend: the first holds.
                if a.value.vertices.len() != b.value.vertices.len() {
                    return a.value.clone();
                }
                Bezier {
                    closed: a.value.closed,
                    vertices: mix(&a.value.vertices, &b.value.vertices),
                    tangents_in: mix(&a.value.tangents_in, &b.value.tangents_in),
                    tangents_out: mix(&a.value.tangents_out, &b.value.tangents_out),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_fixed_value_stays_and_a_number_is_a_value_of_one() {
        assert_eq!(Numbers::parse(Some(&json!({"a": 0, "k": [1, 2]}))).at(5.0), [1.0, 2.0]);
        assert_eq!(Numbers::parse(Some(&json!({"a": 0, "k": 7}))).first_at(0.0, 0.0), 7.0);
        assert_eq!(Numbers::parse(None).first_at(0.0, 3.0), 3.0);
    }

    #[test]
    fn keys_ease_between_them_and_hold_outside() {
        let keys = json!({"a": 1, "k": [
            {"t": 0, "s": [0], "o": {"x": [0], "y": [0]}, "i": {"x": [1], "y": [1]}},
            {"t": 10, "s": [100]}
        ]});
        let value = Numbers::parse(Some(&keys));
        assert_eq!(value.first_at(-1.0, 0.0), 0.0);
        assert!((value.first_at(5.0, 0.0) - 50.0).abs() < 0.5, "a straight line here");
        assert_eq!(value.first_at(11.0, 0.0), 100.0);
        let eased = json!({"a": 1, "k": [
            {"t": 0, "s": [0], "o": {"x": 0.8, "y": 0}, "i": {"x": 1, "y": 1}},
            {"t": 10, "s": [100]}
        ]});
        assert!(Numbers::parse(Some(&eased)).first_at(5.0, 0.0) < 40.0, "a slow start");
    }

    #[test]
    fn a_hold_key_does_not_move_and_old_files_with_e_still_read() {
        let hold = json!({"a": 1, "k": [{"t": 0, "s": [1], "h": 1}, {"t": 10, "s": [2]}]});
        assert_eq!(Numbers::parse(Some(&hold)).first_at(9.0, 0.0), 1.0);
        let old = json!({"a": 1, "k": [
            {"t": 0, "s": [0], "e": [10], "o": {"x": [0], "y": [0]}, "i": {"x": [1], "y": [1]}},
            {"t": 10}
        ]});
        assert_eq!(Numbers::parse(Some(&old)).first_at(20.0, 0.0), 10.0);
    }

    #[test]
    fn a_position_with_tangents_moves_along_its_curve() {
        let curve = json!({"a": 1, "k": [
            {"t": 0, "s": [0, 0], "to": [0, 100], "ti": [0, 100],
             "o": {"x": 0, "y": 0}, "i": {"x": 1, "y": 1}},
            {"t": 10, "s": [100, 0]}
        ]});
        let (x, y) = Numbers::parse(Some(&curve)).pair_at(5.0, (0.0, 0.0));
        assert!((x - 50.0).abs() < 1.0, "{x}");
        assert!(y > 50.0, "it bows out: {y}");
    }

    #[test]
    fn a_shape_blends_its_points_between_keys() {
        let path = |x: f32| json!([{"c": true, "v": [[x, 0.0]], "i": [[0, 0]], "o": [[0, 0]]}]);
        let shape = json!({"a": 1, "k": [
            {"t": 0, "s": path(0.0), "o": {"x": 0, "y": 0}, "i": {"x": 1, "y": 1}},
            {"t": 10, "s": path(10.0)}
        ]});
        let at = Shape::parse(Some(&shape)).at(5.0);
        assert!((at.vertices[0].0 - 5.0).abs() < 0.2, "{:?}", at.vertices);
        assert!(at.closed);
    }
}
