-- SPDX-License-Identifier: MIT
-- Swordman Bash (skill 5) for the Lua hook spike (specs/effects M9, spec §2.11). Our own file:
-- it holds formula facts only (Bash's ratio and hit bonus). Costs, timing and range come from
-- the imported skill data, as for the Rust definitions.
--
-- It returns the same Bash twice, so the spike can compare both against Rust:
--   skills[1]     the declarative definition, written like `Definitions::builtin`;
--   hooks.sm_bash the scripted form, which builds the same effect from ctx.level.

return {
  skills = {
    {
      id = 5,
      effects = {
        {
          kind = "weapon_damage",
          ratio = { base = 100, per_level = 30 },
          hit_pct = { base = 0, per_level = 5 },
          div = 1,
        },
      },
    },
  },

  hooks = {
    sm_bash = function(ctx)
      local lv = ctx.level
      return {
        { kind = "weapon_damage", ratio = 100 + 30 * lv, hit_pct = 5 * lv, div = 1 },
      }
    end,
  },
}
