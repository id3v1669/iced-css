//! Private luminate progress ownership. CSS endpoint/cascade transactions are external.
use super::{
    clock::{AnimationClock, AnimationTiming, TimingError, TimingPhase, TimingWindow},
    linear_progress::{ProgressError, linear_progress},
    motion_sync::MotionSync,
    wakeup::{ProgressWakeup, progress_wakeup},
};
use iced_animate::{Anim, Curve, Easing, Motion, MotionKey};
use iced_css_core::{
    animation::registry::AnimationSelection,
    identity::{IdentityError, InstanceId, SourceChangeError, SourceRef, SourceRegistry},
    values::registry::PropertyId,
};
use std::{
    marker::PhantomData,
    num::{NonZeroU64, NonZeroUsize},
    rc::Rc,
    time::{Duration, Instant},
};
#[path = "source_changes.rs"]
mod source_changes;
pub use source_changes::PreparedSourceBindings;

#[derive(Debug, Clone, Copy)]
pub struct ProgressLimits {
    /// Lifetime address budget; cancellation and collection never replenish it.
    pub addresses: NonZeroU64,
    pub bindings: NonZeroUsize,
    /// Actual Motion entries, including cancelled tracks awaiting collection.
    pub tracks: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineError {
    Identity(IdentityError),
    Sources(SourceChangeError),
    Timing(TimingError),
    Progress(ProgressError),
    Addresses,
    Bindings,
    Tracks,
    Bytes,
    Work,
    Allocation,
    OutgoingNotAdvanced,
}
impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "animation progress owner: {self:?}")
    }
}
impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(e) => Some(e),
            Self::Sources(e) => Some(e),
            Self::Timing(e) => Some(e),
            Self::Progress(e) => Some(e),
            _ => None,
        }
    }
}
struct Binding {
    instance: InstanceId,
    property: PropertyId,
    key: MotionKey,
    progress: Anim<f32>,
    timing: TimingWindow,
}
/// The longest gap replayed tick by tick: 64 steps. Longer gaps re-anchor.
const MAX_REPLAY: Duration = Duration::from_secs(4);
// Every owned start uses Linear. A different easing kind avoids retarget's
// unchanged-curve early return. Targeting the current raw value then settles
// immediately; no interpolated CSS presentation or new address is introduced.
fn quiesce(motion: &Motion, binding: &Binding) {
    let current = binding.progress.get();
    drop(motion.to(
        binding.key,
        Curve::ease(Easing::EaseInOut, Duration::ZERO),
        current,
    ));
}
/// A low-level host building block, not an application CSS request dispatcher.
/// Taking the registry by value couples token scope and engine lifetime. No API
/// extracts that registry, clones the engine or exposes live progress handles.
///
/// ```compile_fail
/// use iced_css::animation::engine::ProgressEngine;
/// fn escape(engine: &ProgressEngine) { let _ = engine.motion.clone(); }
/// ```
/// The engine stays on its owning runner thread; SourceRef tokens may cross
/// threads through Messages, but live engine access may not.
/// ```compile_fail
/// use iced_css::animation::engine::ProgressEngine;
/// fn require_send<T: Send>() {}
/// require_send::<ProgressEngine>();
/// ```
pub struct ProgressEngine {
    sources: SourceRegistry,
    motion: Motion,
    sync: MotionSync,
    clock: AnimationClock,
    next: Option<NonZeroU64>,
    limits: ProgressLimits,
    bindings: Vec<Binding>,
    runner_thread: PhantomData<Rc<()>>,
}
impl ProgressEngine {
    pub(crate) fn live_source_count(&self) -> usize {
        self.sources.live_count()
    }
    pub fn new(sources: SourceRegistry, limits: ProgressLimits) -> Self {
        Self {
            sources,
            motion: Motion::new(),
            sync: MotionSync::default(),
            clock: AnimationClock::default(),
            next: NonZeroU64::new(1),
            limits,
            bindings: Vec::new(),
            runner_thread: PhantomData,
        }
    }
    /// Called by source publication; registration alone creates no animation.
    pub fn register_source(&mut self) -> Result<SourceRef, EngineError> {
        self.sources.insert().map_err(EngineError::Identity)
    }
    pub fn instance(&self, source: &SourceRef) -> Result<InstanceId, EngineError> {
        self.sources.validate(source).map_err(EngineError::Identity)
    }
    pub fn remove_source(&mut self, source: &SourceRef) -> Result<(), EngineError> {
        let instance = self.instance(source)?;
        self.check_work(self.bindings.len())?;
        self.sources.remove(source).map_err(EngineError::Identity)?;
        let motion = &self.motion;
        self.bindings.retain(|binding| {
            if binding.instance == instance {
                quiesce(motion, binding);
                false
            } else {
                true
            }
        });
        Ok(())
    }
    fn check_work(&self, work: usize) -> Result<(), EngineError> {
        if work > self.limits.work.get() {
            Err(EngineError::Work)
        } else {
            Ok(())
        }
    }
    /// Advance all tracks at one nondecreasing host timestamp. This never starts
    /// a track. The host coordinates outgoing dependency sampling after advance.
    ///
    /// A gap longer than [`MAX_REPLAY`] is not replayed step by step, which would
    /// cost one dependency tick per 62.5 ms of it (a long delay, a suspended
    /// application). The dependency is ticked once and every owned track restarts
    /// there from its own timing window: the rest of its delay, or the rest of its
    /// linear run from the progress wall-clock time gives it.
    pub fn advance(&mut self, now: Instant) -> Instant {
        let accepted = self.clock.accept(now);
        if self.sync.backlog(accepted) > MAX_REPLAY {
            self.sync.jump(&self.motion, accepted);
            self.reanchor(accepted);
        } else {
            self.sync.synchronize(&self.motion, accepted);
        }
        accepted
    }
    fn reanchor(&mut self, now: Instant) {
        for binding in &mut self.bindings {
            let window = binding.timing;
            match window.phase_at(now) {
                TimingPhase::Delayed => {
                    let run = window.ends_at().duration_since(window.starts_at());
                    let wait = window.starts_at().duration_since(now);
                    let curve = Curve::ease(Easing::Linear, run).delayed(wait);
                    binding.progress = self.motion.play(binding.key, curve, 0.0_f32, 1.0);
                }
                TimingPhase::Active { elapsed, duration } => {
                    // In the dependency's own terms: it runs no ease shorter than
                    // `f32::EPSILON` seconds, and `linear_progress` scales its raw
                    // sample back by that floor. Restart on the floored run, so the
                    // same correction still gives wall-clock progress.
                    let floor = duration.as_secs_f32().max(f32::EPSILON);
                    let done = elapsed.as_secs_f32() / floor;
                    let rest = if duration.as_secs_f32() < f32::EPSILON {
                        Duration::from_secs_f32(floor).saturating_sub(elapsed)
                    } else {
                        duration - elapsed
                    };
                    let curve = Curve::ease(Easing::Linear, rest);
                    binding.progress =
                        self.motion
                            .play(binding.key, curve, done.clamp(0.0, 1.0), 1.0);
                }
                // Sampling a complete window reads no track: let it settle.
                TimingPhase::Complete => quiesce(&self.motion, binding),
            }
        }
    }
    /// Ticks handed to the dependency so far; a bounded-work diagnostic.
    pub fn synchronization_ticks(&self) -> u64 {
        self.sync.ticks()
    }
    /// Start a prevalidated set of progress tracks atomically with respect to
    /// returned admission errors. The caller must validate CSS endpoints, target
    /// changes, precedence and publication, and capture outgoing samples first.
    /// This boundary does not promise recovery from dependency allocation panics.
    pub fn start_progress(
        &mut self,
        source: &SourceRef,
        selected: &AnimationSelection,
        timing: AnimationTiming,
        now: Instant,
    ) -> Result<Instant, EngineError> {
        Ok(self
            .prepare_progress(source, selected, timing, now)?
            .commit())
    }
    /// Reserve and validate without advancing time or installing starts. The
    /// returned guard holds exclusive engine access until commit or abandonment.
    pub fn prepare_progress<'engine, 'selection>(
        &'engine mut self,
        source: &SourceRef,
        selected: &'selection AnimationSelection,
        timing: AnimationTiming,
        now: Instant,
    ) -> Result<PreparedProgressBatch<'engine, 'selection>, EngineError> {
        let instance = self.instance(source)?;
        let accepted = self.clock.last().map_or(now, |last| last.max(now));
        let window = timing.accept(accepted).map_err(EngineError::Timing)?;
        // Two full registry scans, two searches per selected property, and moving
        // stored bindings. Source-registry and dependency internals are separate.
        let total_bound = self
            .bindings
            .len()
            .checked_add(selected.len())
            .ok_or(EngineError::Work)?;
        let work = total_bound
            .checked_mul(selected.len())
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| n.checked_add(total_bound))
            .and_then(|n| {
                PropertyId::all()
                    .len()
                    .checked_mul(2)
                    .and_then(|scans| n.checked_add(scans))
            })
            .ok_or(EngineError::Work)?;
        self.check_work(work)?;
        let new = selected
            .properties()
            .filter(|property| {
                !self
                    .bindings
                    .iter()
                    .any(|b| b.instance == instance && b.property == *property)
            })
            .count();
        let total = self
            .bindings
            .len()
            .checked_add(new)
            .ok_or(EngineError::Bindings)?;
        if total > self.limits.bindings.get() {
            return Err(EngineError::Bindings);
        }
        let tracks = self
            .motion
            .track_count()
            .checked_add(new)
            .ok_or(EngineError::Tracks)?;
        if tracks > self.limits.tracks.get() {
            return Err(EngineError::Tracks);
        }
        let post_next = if new == 0 {
            self.next
        } else {
            let first = self.next.ok_or(EngineError::Addresses)?;
            let count = u64::try_from(new).map_err(|_| EngineError::Addresses)?;
            let last = first
                .get()
                .checked_add(count - 1)
                .ok_or(EngineError::Addresses)?;
            if last > self.limits.addresses.get() {
                return Err(EngineError::Addresses);
            }
            if last == self.limits.addresses.get() {
                None
            } else {
                last.checked_add(1).and_then(NonZeroU64::new)
            }
        };
        let bytes = total
            .checked_mul(size_of::<Binding>())
            .ok_or(EngineError::Bytes)?;
        if bytes > self.limits.retained_bytes.get() || bytes > isize::MAX as usize {
            return Err(EngineError::Bytes);
        }
        // Reserve a replacement, so a failed capacity check drops the allocation
        // without changing any stored binding or address state.
        let mut staged = Vec::new();
        staged
            .try_reserve_exact(total)
            .map_err(|_| EngineError::Allocation)?;
        if staged
            .capacity()
            .checked_mul(size_of::<Binding>())
            .is_none_or(|n| n > self.limits.retained_bytes.get())
        {
            return Err(EngineError::Bytes);
        }
        Ok(PreparedProgressBatch {
            engine: self,
            selected,
            instance,
            timing,
            window,
            accepted,
            post_next,
            staged,
            advanced: false,
        })
    }
    /// None means no presentation override; use the current underlying CSS value.
    /// Reading a completed binding does not keep scheduling it or restart it.
    pub fn sample(
        &self,
        source: &SourceRef,
        property: PropertyId,
    ) -> Result<Option<f32>, EngineError> {
        let instance = self.instance(source)?;
        self.check_work(self.bindings.len())?;
        let Some(binding) = self
            .bindings
            .iter()
            .find(|b| b.instance == instance && b.property == property)
        else {
            return Ok(None);
        };
        let now = self
            .clock
            .last()
            .expect("clock initialized before installing bindings");
        let phase = binding.timing.phase_at(now);
        if phase == TimingPhase::Complete {
            return Ok(None);
        }
        linear_progress(phase, binding.progress.get())
            .map(Some)
            .map_err(EngineError::Progress)
    }
    /// The engine's time: the last it advanced to.
    #[must_use]
    pub fn now(&self) -> Option<Instant> {
        self.clock.last()
    }

    /// Where a binding's timing window is at the engine's time, and the instant it ends:
    /// for telling when a delay is over and a run complete. `None` without the binding.
    pub fn phase(
        &self,
        source: &SourceRef,
        property: PropertyId,
    ) -> Result<Option<(TimingPhase, Instant)>, EngineError> {
        let instance = self.instance(source)?;
        self.check_work(self.bindings.len())?;
        let Some(binding) = self
            .bindings
            .iter()
            .find(|b| b.instance == instance && b.property == property)
        else {
            return Ok(None);
        };
        let Some(now) = self.clock.last() else {
            return Ok(None);
        };
        Ok(Some((
            binding.timing.phase_at(now),
            binding.timing.ends_at(),
        )))
    }
    pub fn cancel(
        &mut self,
        source: &SourceRef,
        selected: &AnimationSelection,
    ) -> Result<(), EngineError> {
        let instance = self.instance(source)?;
        self.check_work(self.bindings.len())?;
        let motion = &self.motion;
        self.bindings.retain(|binding| {
            if binding.instance == instance && selected.rule(binding.property).is_some() {
                quiesce(motion, binding);
                false
            } else {
                true
            }
        });
        Ok(())
    }
    /// Whether a live owned binding still needs elapsed-time advancement. Orphan
    /// luminate tracks left after cancellation do not request CSS redraws here.
    pub fn is_animating(&self) -> bool {
        self.next_wakeup() != ProgressWakeup::Idle
    }
    /// Active intervals need frame advancement; pure delays can wait for their
    /// earliest start. This reads timing metadata without touching live tracks.
    pub fn next_wakeup(&self) -> ProgressWakeup {
        self.clock.last().map_or(ProgressWakeup::Idle, |now| {
            progress_wakeup(now, self.bindings.iter().map(|binding| binding.timing))
        })
    }
    /// Exactly one call per accepted binding build, coordinated by the host.
    pub fn accepted_build(&mut self) {
        self.motion.end_build();
        self.motion.collect();
    }
}

/// Reserved progress starts and an exclusive outgoing-capture window. Dropping
/// before advancement changes no clock or binding. Dropping after explicit
/// advancement leaves shared time advanced but installs no starts.
///
/// A prepared batch is consumed exactly once:
/// ```compile_fail
/// use iced_css::animation::engine::PreparedProgressBatch;
/// fn twice(batch: PreparedProgressBatch<'_, '_>) {
///     batch.commit();
///     batch.commit();
/// }
/// ```
/// Engine mutation cannot interleave with an outstanding preparation:
/// ```compile_fail
/// use iced_css::animation::{engine::ProgressEngine, clock::AnimationTiming};
/// use iced_css_core::{identity::SourceRef, animation::registry::AnimationSelection};
/// use std::time::Instant;
/// fn interleave(engine: &mut ProgressEngine, source: &SourceRef,
///              selection: &AnimationSelection, timing: AnimationTiming, now: Instant) {
///     let batch = engine.prepare_progress(source, selection, timing, now).unwrap();
///     engine.advance(now);
///     batch.commit();
/// }
/// ```
#[must_use = "commit the prepared starts or drop the reservation"]
pub struct PreparedProgressBatch<'engine, 'selection> {
    engine: &'engine mut ProgressEngine,
    selected: &'selection AnimationSelection,
    instance: InstanceId,
    timing: AnimationTiming,
    window: TimingWindow,
    accepted: Instant,
    post_next: Option<NonZeroU64>,
    staged: Vec<Binding>,
    advanced: bool,
}
impl PreparedProgressBatch<'_, '_> {
    /// Advance once at the reserved time; repeated calls retain that timestamp.
    pub fn advance_outgoing(&mut self) -> Instant {
        self.engine.advance(self.accepted);
        self.advanced = true;
        self.accepted
    }
    /// Read an existing owner's progress for CSS sampling under outgoing context.
    pub fn sample_outgoing(
        &self,
        source: &SourceRef,
        property: PropertyId,
    ) -> Result<Option<f32>, EngineError> {
        self.engine.instance(source)?;
        if !self.advanced {
            return Err(EngineError::OutgoingNotAdvanced);
        }
        self.engine.sample(source, property)
    }
    /// Consume the reservation and install all starts without further returned
    /// admission errors. Luminate's internal allocations may still panic.
    pub fn commit(self) -> Instant {
        let Self {
            engine,
            selected,
            instance,
            timing,
            window,
            accepted,
            post_next,
            mut staged,
            ..
        } = self;
        engine.advance(accepted);
        staged.append(&mut engine.bindings);
        engine.bindings = staged;
        let mut next = engine.next;
        let curve = Curve::ease(Easing::Linear, timing.duration()).delayed(timing.delay());
        for property in selected.properties() {
            if let Some(binding) = engine
                .bindings
                .iter_mut()
                .find(|b| b.instance == instance && b.property == property)
            {
                binding.progress = engine.motion.play(binding.key, curve, 0.0_f32, 1.0);
                binding.timing = window;
            } else {
                let raw = next.expect("all new addresses preflighted");
                next = raw.get().checked_add(1).and_then(NonZeroU64::new);
                let key = MotionKey::from_raw(raw.get());
                engine.bindings.push(Binding {
                    instance,
                    property,
                    key,
                    progress: engine.motion.play(key, curve, 0.0_f32, 1.0),
                    timing: window,
                });
            }
        }
        engine.next = post_next;
        engine.sync.restart(&engine.motion, accepted);
        accepted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn engine() -> ProgressEngine {
        ProgressEngine::new(
            SourceRegistry::new(NonZeroU64::new(10).unwrap()),
            ProgressLimits {
                addresses: NonZeroU64::new(u64::MAX).unwrap(),
                bindings: NonZeroUsize::new(10).unwrap(),
                tracks: NonZeroUsize::new(100).unwrap(),
                retained_bytes: NonZeroUsize::new(1000000).unwrap(),
                work: NonZeroUsize::new(100000).unwrap(),
            },
        )
    }
    fn selection(name: &str) -> AnimationSelection {
        AnimationSelection::new(
            &[PropertyId::lookup(name).unwrap()],
            NonZeroUsize::new(1000).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn maximum_address_does_not_wrap_and_existing_owner_can_restart() {
        let mut engine = engine();
        engine.next = NonZeroU64::new(u64::MAX);
        let source = engine.register_source().unwrap();
        let now = Instant::now();
        let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
        engine
            .start_progress(&source, &selection("color"), timing, now)
            .unwrap();
        assert_eq!(engine.bindings[0].key, MotionKey::from_raw(u64::MAX));
        assert_eq!(engine.next, None);
        assert_eq!(engine.motion.track_count(), 1);
        assert_eq!(
            engine.start_progress(&source, &selection("margin-top"), timing, now),
            Err(EngineError::Addresses)
        );
        engine
            .start_progress(&source, &selection("color"), timing, now)
            .unwrap();
        assert_eq!(engine.motion.track_count(), 1);
        engine.remove_source(&source).unwrap();
        let fresh = engine.register_source().unwrap();
        assert_eq!(
            engine.start_progress(&fresh, &selection("color"), timing, now),
            Err(EngineError::Addresses)
        );
        assert_eq!(engine.motion.track_count(), 1);
    }
    #[test]
    fn source_registration_and_binding_builds_do_not_create_tracks() {
        let mut engine = engine();
        let source = engine.register_source().unwrap();
        for _ in 0..5 {
            engine.accepted_build();
        }
        assert_eq!(
            engine
                .sample(&source, PropertyId::lookup("color").unwrap())
                .unwrap(),
            None
        );
        assert_eq!(engine.motion.track_count(), 0);
        assert_eq!(engine.next, NonZeroU64::new(1));
    }
    #[test]
    fn cancelled_and_removed_tracks_collect_after_build_grace_without_extra_ticks() {
        for remove in [false, true] {
            for delay in [Duration::ZERO, Duration::from_secs(10)] {
                let mut engine = engine();
                let source = engine.register_source().unwrap();
                let now = Instant::now();
                engine
                    .start_progress(
                        &source,
                        &selection("color"),
                        AnimationTiming::new(Duration::from_secs(1), delay),
                        now,
                    )
                    .unwrap();
                engine.advance(now + Duration::from_millis(250));
                if remove {
                    engine.remove_source(&source).unwrap();
                } else {
                    engine.cancel(&source, &selection("color")).unwrap();
                }
                assert!(!engine.is_animating());
                assert_eq!(engine.motion.track_count(), 1);
                for expected in [1, 1, 0] {
                    engine.accepted_build();
                    assert_eq!(engine.motion.track_count(), expected);
                }
                assert_eq!(engine.next, NonZeroU64::new(2));
            }
        }
    }
    #[test]
    fn synchronization_issues_bounded_sub_steps_and_none_while_idle() {
        let mut engine = engine();
        let source = engine.register_source().unwrap();
        let color = PropertyId::lookup("color").unwrap();
        let now = Instant::now();
        engine.advance(now);
        assert_eq!(
            engine.synchronization_ticks(),
            0,
            "idle advances issue no tick"
        );
        engine
            .start_progress(
                &source,
                &selection("color"),
                AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
                now,
            )
            .unwrap();
        assert_eq!(
            engine.synchronization_ticks(),
            1,
            "one zero-spend restart tick"
        );
        engine.advance(now + Duration::from_millis(250));
        assert_eq!(
            engine.synchronization_ticks(),
            5,
            "four exact 62.5 ms steps"
        );
        assert_eq!(engine.sample(&source, color).unwrap(), Some(0.25));
        engine.advance(now + Duration::from_millis(260));
        assert_eq!(
            engine.synchronization_ticks(),
            6,
            "a 10 ms delta is one tick"
        );
        let sample = engine.sample(&source, color).unwrap().unwrap();
        assert!((sample - 0.26).abs() <= 2.0 * f32::EPSILON, "{sample}");
        engine.advance(now + Duration::from_secs(3));
        // 0.74 s remain: eleven full steps reach 0.9475, the twelfth settles the
        // track but reports movement, and the thirteenth observes rest (the
        // dependency's own was_moving rule); the remaining span is not ticked.
        assert_eq!(engine.synchronization_ticks(), 19);
        assert_eq!(engine.sample(&source, color).unwrap(), None);
        assert!(!engine.is_animating());
        engine.advance(now + Duration::from_secs(3600));
        assert_eq!(engine.synchronization_ticks(), 19, "idle again: no tick");
    }
    #[test]
    fn a_long_gap_re_anchors_tracks_instead_of_replaying_every_step() {
        let color = PropertyId::lookup("color").unwrap();
        let day = Duration::from_secs(86_400);
        let run = Duration::from_secs(10);
        // Through a one-day delay and half of the run in one advance.
        let mut engine = engine();
        let source = engine.register_source().unwrap();
        let now = Instant::now();
        engine
            .start_progress(
                &source,
                &selection("color"),
                AnimationTiming::new(run, day),
                now,
            )
            .unwrap();
        engine.advance(now + day / 2);
        assert_eq!(engine.sample(&source, color).unwrap(), Some(0.0));
        engine.advance(now + day + run / 2);
        assert!(engine.synchronization_ticks() < 10, "bounded catch-up");
        let sample = engine.sample(&source, color).unwrap().unwrap();
        assert!((sample - 0.5).abs() <= 1e-5, "{sample}");
        // The re-anchored track keeps wall-clock time on ordinary frames.
        engine.advance(now + day + run / 2 + Duration::from_secs(1));
        let sample = engine.sample(&source, color).unwrap().unwrap();
        assert!((sample - 0.6).abs() <= 1e-5, "{sample}");
        // The delay runs out between frames: its start is where it should be.
        engine.advance(now + day + run * 3 / 4);
        let sample = engine.sample(&source, color).unwrap().unwrap();
        assert!((sample - 0.75).abs() <= 1e-5, "{sample}");
        // Past the end in one jump: complete, and nothing left to tick.
        engine.advance(now + day * 8);
        assert_eq!(engine.sample(&source, color).unwrap(), None);
        assert!(!engine.is_animating());
        let ticks = engine.synchronization_ticks();
        engine.advance(now + day * 9);
        engine.advance(now + day * 10);
        assert!(
            engine.synchronization_ticks() <= ticks + 2,
            "settles to idle"
        );
    }
    #[test]
    fn a_long_gap_into_a_run_below_the_ease_floor_keeps_wall_clock_progress() {
        let color = PropertyId::lookup("color").unwrap();
        let mut engine = engine();
        let source = engine.register_source().unwrap();
        let now = Instant::now();
        let delay = Duration::from_secs(10);
        let timing = AnimationTiming::new(Duration::from_nanos(10), delay);
        engine
            .start_progress(&source, &selection("color"), timing, now)
            .unwrap();
        engine.advance(now + delay + Duration::from_nanos(5));
        let sample = engine.sample(&source, color).unwrap().unwrap();
        assert!((sample - 0.5).abs() <= 1e-5, "{sample}");
    }
    #[test]
    fn start_at_the_settling_instant_uses_a_one_nanosecond_restart() {
        let mut engine = engine();
        let source = engine.register_source().unwrap();
        let now = Instant::now();
        let timing = AnimationTiming::new(Duration::from_secs(1), Duration::ZERO);
        engine
            .start_progress(&source, &selection("color"), timing, now)
            .unwrap();
        engine.advance(now + Duration::from_secs(1));
        assert_eq!(
            engine.synchronization_ticks(),
            17,
            "restart plus sixteen steps"
        );
        assert!(!engine.is_animating());
        // The settling step reported movement; this tick observes rest exactly
        // at the instant the next batch is accepted.
        let rest = now + Duration::from_millis(1100);
        engine.advance(rest);
        assert_eq!(engine.synchronization_ticks(), 18);
        engine
            .start_progress(&source, &selection("margin-top"), timing, rest)
            .unwrap();
        assert_eq!(
            engine.synchronization_ticks(),
            19,
            "restart one nanosecond later"
        );
        engine.advance(rest + Duration::from_millis(250));
        assert_eq!(engine.synchronization_ticks(), 23);
        // The new track trails wall-clock by one nanosecond, below f32 resolution here.
        assert_eq!(
            engine
                .sample(&source, PropertyId::lookup("margin-top").unwrap())
                .unwrap(),
            Some(0.25)
        );
    }
    #[test]
    fn delayed_start_counts_as_moving_and_burns_its_delay_on_wall_clock() {
        let mut engine = engine();
        let source = engine.register_source().unwrap();
        let color = PropertyId::lookup("color").unwrap();
        let now = Instant::now();
        engine
            .start_progress(
                &source,
                &selection("color"),
                AnimationTiming::new(Duration::from_secs(1), Duration::from_secs(1)),
                now,
            )
            .unwrap();
        engine.advance(now + Duration::from_millis(500));
        assert_eq!(engine.synchronization_ticks(), 9, "holding still ticks");
        assert_eq!(engine.sample(&source, color).unwrap(), Some(0.0));
        engine.advance(now + Duration::from_millis(1250));
        assert_eq!(engine.synchronization_ticks(), 21);
        assert_eq!(engine.sample(&source, color).unwrap(), Some(0.25));
    }
    #[test]
    fn cancellation_quiesces_only_its_private_track_at_the_current_raw_sample() {
        let mut engine = engine();
        let source = engine.register_source().unwrap();
        let now = Instant::now();
        let both = AnimationSelection::new(
            &[
                PropertyId::lookup("color").unwrap(),
                PropertyId::lookup("margin-top").unwrap(),
            ],
            NonZeroUsize::new(1000).unwrap(),
        )
        .unwrap();
        engine
            .start_progress(
                &source,
                &both,
                AnimationTiming::new(Duration::from_secs(1), Duration::ZERO),
                now,
            )
            .unwrap();
        engine.advance(now + Duration::from_millis(250));
        let key = engine
            .bindings
            .iter()
            .find(|b| b.property == PropertyId::lookup("color").unwrap())
            .unwrap()
            .key;
        let observer = engine.motion.get::<f32>(key).unwrap();
        assert_eq!(observer.get(), 0.25);
        engine.cancel(&source, &selection("color")).unwrap();
        assert!(!observer.is_animating());
        assert_eq!(observer.get(), 0.25);
        assert_eq!(
            engine
                .sample(&source, PropertyId::lookup("color").unwrap())
                .unwrap(),
            None
        );
        assert!(engine.is_animating());
        engine.advance(now + Duration::from_millis(500));
        assert_eq!(
            engine
                .sample(&source, PropertyId::lookup("margin-top").unwrap())
                .unwrap(),
            Some(0.5)
        );
        assert_eq!(observer.get(), 0.25);
        drop(observer);
        for _ in 0..3 {
            engine.accepted_build();
        }
        assert_eq!(engine.motion.track_count(), 1);
    }
}
