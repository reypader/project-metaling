-- SPDX-License-Identifier: MIT
-- The `itemheal` hook for the Lua hook spike (specs/effects M9). Our own file, written from the
-- formula facts in catalog `mechanics/inventory.md` §7.4. It must draw the same rolls, in the
-- same order, as the Rust `item_heal`: the HP roll, then the SP roll, each only when its range
-- has more than one value.
--
-- ctx.args = { hp_min, hp_max, sp_min, sp_max }, inclusive. The potion values themselves come
-- from the imported item data.

local SM_RECOVERY = 4
local MG_SRECOVERY = 9
local AM_LEARNINGPOTION = 227

-- Script rand(min, max), inclusive. A constant draws nothing.
local function rand(ctx, min, max)
  if min < 0 or max < min then
    error(("item_heal: bad range %d..=%d"):format(min, max))
  end
  if max == min then
    return min
  end
  return min + ctx:roll(max - min + 1)
end

-- value * bonus / 100, kept only when the bonus isn't 100 and the result is higher.
local function scaled(value, bonus)
  local boosted = value * bonus // 100
  if bonus ~= 100 and boosted > value then
    return boosted
  end
  return value
end

return {
  hooks = {
    item_heal = function(ctx)
      local args = ctx.args
      local hp = rand(ctx, args[1], args[2])
      local sp = rand(ctx, args[3], args[4])
      local learning = 5 * ctx:skill_level(AM_LEARNINGPOTION)
      local caster = ctx.caster
      local hp_bonus = 100 + 2 * caster.vit + 10 * ctx:skill_level(SM_RECOVERY)
      local sp_bonus = 100 + 2 * caster.int + 10 * ctx:skill_level(MG_SRECOVERY)
      return {
        {
          kind = "heal",
          to = "caster",
          hp = scaled(hp, hp_bonus + learning),
          sp = scaled(sp, sp_bonus + learning),
          show = "silent",
        },
      }
    end,
  },
}
