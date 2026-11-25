-- What the page gives the finger and the ear, and only where nothing on the screen says it already or
-- the person is not looking: a light tick on the trackpad as the pointer goes from one line of the list
-- to another, a soft knock when the edge beside the list is dragged into one of its ends, a sound when
-- Linear did not take a change (the line has gone back to what it was, maybe while the person looked
-- elsewhere), and one when an agent stops (it is done, or it waits for the person, in a pane that is not
-- this one). Nothing else sounds: a press, a pick or a key is the person's own and they know of it.
--
-- The terminal's kit runs the page, and one older than these has none of them: each is asked of it first
-- (`pencere.has`), and where it does not have one nothing is said. A host vibrates only while a finger is
-- on the trackpad, and the person's own switches silence both.

local kit = require("../lib/kit")

local S = kit.S

local feel = {}

-- The kit's ready vibrations and sounds, by their names. The ready patterns are light (`tick` is the
-- lightest the kit has), so none is given a strength of its own.
local OVER_LINE = "tick"
local AT_END = "bump"
local REFUSED = "error"
local STOPPED = "notify"

-- Seconds within which one agent does not sound again: an agent that waits for the person and redraws
-- its prompt writes, is at work for a moment, and stops again.
local STOPPED_AGAIN = 30

local F = {
	-- The key of the line the pointer was last on, nil off the lines.
	line = nil,
	-- The end of the edge it was dragged into ("least" or "most"), nil between them.
	edge = nil,
	-- When each agent last sounded, by the Linear id of its issue.
	stopped = {},
}

local function vibration(name)
	if kit.has("haptic." .. name) then
		return { haptic = name }
	end
	return nil
end

local function sound(name)
	if kit.has("sound." .. name) then
		return { sound = name }
	end
	return nil
end

-- The parts of a line the pointer can be on, and how much of the name after the prefix is the line's
-- key: its box of selection and a group's arrow are named by the key; an agent's mark is named by the
-- issue's id, and is on a line the pointer is already on.
local OF_LINE = { ["row:"] = true, ["check:"] = true, ["arrow:"] = true }
local IN_LINE = { ["agent:"] = true, ["agent-idle:"] = true }

-- The key of the line `over` (what `pencere.layout.hit` says is under the pointer) is of; false where it
-- is a part of a line that does not say which (the pointer is still on the line it was on); nil off the
-- lines.
local function line_of(over)
	if over == nil then
		return nil
	end
	local colon = string.find(over, ":", 1, true)
	if colon == nil then
		return nil
	end
	local prefix = string.sub(over, 1, colon)
	if OF_LINE[prefix] then
		return string.sub(over, colon + 1)
	elseif IN_LINE[prefix] then
		return false
	end
	return nil
end

-- The pointer moved, no button held, over the list the person can use: `over` is what is under it. A
-- tick where it came onto another line, once however it moves on that line; coming back onto a line from
-- off the lines is coming onto one, and ticks too.
function feel.moved(over)
	local line = line_of(over)
	if line == false then
		return nil
	end
	local came = line ~= nil and line ~= F.line
	F.line = line
	if came then
		return vibration(OVER_LINE)
	end
	return nil
end

-- The pointer left the page, or the list is not where it is: the next line it comes onto ticks.
function feel.off()
	F.line = nil
end

-- The edge beside the list was dragged (`final` false) or let go and stepped by keys (`final` true) to
-- `size`, between `least` and `most`: a knock once each time a drag takes it into one of its ends. Keys
-- have no finger on the trackpad, and an edge whose ends are one has nothing to run into.
function feel.edge(size, least, most, final)
	if final then
		F.edge = nil
		return nil
	end
	if most - least < 1 then
		F.edge = nil
		return nil
	end
	local at = nil
	if size <= least + 0.5 then
		at = "least"
	elseif size >= most - 0.5 then
		at = "most"
	end
	local reply = nil
	if at ~= nil and at ~= F.edge then
		reply = vibration(AT_END)
	end
	F.edge = at
	return reply
end

-- Linear did not take a change the person made (one call, however many issues it was of).
function feel.refused()
	return sound(REFUSED)
end

-- The topic's agents changed from `before` to `now` (`working`, by the Linear id of the issue): a sound
-- where one that was at work has stopped. Not for the first that are told, nor for an agent found
-- stopped already (the pen began again, or found another pen's), nor again for one within
-- `STOPPED_AGAIN` seconds of `at` (the clock of `render`).
function feel.agents(before, now, at)
	if now == nil then
		return nil
	end
	for id in F.stopped do
		if now[id] == nil then
			F.stopped[id] = nil
		end
	end
	if before == nil then
		return nil
	end
	local stopped = false
	for id, working in now do
		local was = before[id]
		if working.idle == true and was ~= nil and was.idle ~= true then
			local last = F.stopped[id]
			if last == nil or at - last >= STOPPED_AGAIN then
				F.stopped[id] = at
				stopped = true
			end
		end
	end
	if stopped then
		return sound(STOPPED)
	end
	return nil
end

return feel
