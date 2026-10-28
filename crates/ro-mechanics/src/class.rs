/// What a job is, as far as the formulas care. `ro-engine` derives it from the job's class
/// mapid (catalog `mechanics/equipment.md` "Job id -> mapid").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClassFlags {
    /// Transcendent (`JOBL_UPPER`): ×1.25 MaxHP/MaxSP, the transcendent stat cap.
    pub upper: bool,
    /// Baby (`JOBL_BABY`).
    pub baby: bool,
    /// 3rd class (`JOBL_THIRD`; 4th classes carry it too): the 3rd-class ASPD cap.
    pub third: bool,
    /// Primary 4th class (`pc_is_primary_fourth`: the 2-1/2-2 lines' 4th classes, not Hyper
    /// Novice, Sky Emperor or Soul Ascetic): ×1.25 MaxHP/MaxSP like transcendent classes.
    /// Captured on a Shadow Cross and, without it, a Hyper Novice (catalog `mechanics/stats.md`
    /// "4th-class MaxHP/MaxSP", "Hyper Novice").
    pub fourth: bool,
    /// A job with trait stats (`pc_is_trait_job`): the 4th-class stat cap.
    pub trait_job: bool,
    /// Summoner (Doram) line.
    pub summoner: bool,
    /// Kagerou/Oboro or Rebellion lines (the "extended" classes).
    pub extended: bool,
}
