//! CSS Easing Functions 1: what a transition's `transition-timing-function` makes of its
//! linear progress.

/// Where `steps()` jumps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepPosition {
    JumpStart,
    JumpEnd,
    JumpNone,
    JumpBoth,
}

/// An easing function, its keywords written as the functions they name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssEasing {
    Linear,
    /// x1, y1, x2, y2, both x within 0 and 1.
    CubicBezier([f32; 4]),
    /// A positive count, and where it jumps.
    Steps(u32, StepPosition),
}

impl CssEasing {
    /// `ease`, the initial easing.
    pub const EASE: Self = Self::CubicBezier([0.25, 0.1, 0.25, 1.0]);

    /// The output progress at input progress `x`, from 0 to 1.
    #[must_use]
    pub fn value(self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        match self {
            Self::Linear => x,
            Self::CubicBezier([x1, y1, x2, y2]) => bezier(x1, y1, x2, y2, x),
            Self::Steps(count, position) => steps(count, position, x),
        }
    }
}

/// One coordinate of a cubic Bézier from 0 to 1 through control points `a` and `b`.
fn curve(a: f32, b: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
}

/// Its slope.
fn slope(a: f32, b: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
}

/// 2.2: the curve's y where its x is `x`: Newton's method, then bisection should that
/// stall (x is monotonic in t, both x controls lying within 0 and 1).
fn bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let mut t = x;
    for _ in 0..8 {
        let error = curve(x1, x2, t) - x;
        if error.abs() < 1e-6 {
            return curve(y1, y2, t);
        }
        let derivative = slope(x1, x2, t);
        if derivative.abs() < 1e-6 {
            break;
        }
        t = (t - error / derivative).clamp(0.0, 1.0);
    }
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    t = x;
    for _ in 0..40 {
        let at = curve(x1, x2, t);
        if (at - x).abs() < 1e-6 {
            break;
        }
        if at < x {
            low = t;
        } else {
            high = t;
        }
        t = (low + high) / 2.0;
    }
    curve(y1, y2, t)
}

/// 3.1.1: the step function.
fn steps(count: u32, position: StepPosition, x: f32) -> f32 {
    let count = count.max(1) as f32;
    let mut step = (x * count).floor();
    if matches!(position, StepPosition::JumpStart | StepPosition::JumpBoth) {
        step += 1.0;
    }
    let jumps = match position {
        StepPosition::JumpNone => count - 1.0,
        StepPosition::JumpBoth => count + 1.0,
        _ => count,
    };
    (step.min(jumps) / jumps.max(1.0)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_curves_and_steps_follow_css_easing() {
        let ease = CssEasing::EASE;
        assert_eq!(ease.value(0.0), 0.0);
        assert!((ease.value(1.0) - 1.0).abs() < 1e-4);
        // `ease` is well past halfway at half time.
        assert!((ease.value(0.5) - 0.8024).abs() < 1e-3);
        let linear_curve = CssEasing::CubicBezier([0.0, 0.0, 1.0, 1.0]);
        assert!((linear_curve.value(0.3) - 0.3).abs() < 1e-4);
        let ease_in = CssEasing::CubicBezier([0.42, 0.0, 1.0, 1.0]);
        assert!(ease_in.value(0.25) < 0.25);
        let end = CssEasing::Steps(4, StepPosition::JumpEnd);
        assert_eq!(
            [0.0, 0.2, 0.25, 0.99, 1.0].map(|x| end.value(x)),
            [0.0, 0.0, 0.25, 0.75, 1.0]
        );
        let start = CssEasing::Steps(1, StepPosition::JumpStart);
        assert_eq!(start.value(0.0), 1.0);
        let none = CssEasing::Steps(3, StepPosition::JumpNone);
        assert_eq!([0.0, 0.5, 1.0].map(|x| none.value(x)), [0.0, 0.5, 1.0]);
        let both = CssEasing::Steps(1, StepPosition::JumpBoth);
        assert_eq!([0.0, 0.6, 1.0].map(|x| both.value(x)), [0.5, 0.5, 1.0]);
    }
}
