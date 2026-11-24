-- The small pictures of the list: the circle of a state that fills as the work goes (the way Linear
-- draws it), the bars of a priority, the arrow of a group, and the dot of a label. They are drawn
-- with the kit's canvas, in the roles of the theme, so that they follow its colours.

local kit = require("../lib/kit")

local b = kit.b

local icons = {}

local SIDE = 14

-- A slice of a circle (`from` to `to`, degrees clockwise from the top) as a closed shape from its
-- middle: a pie that fills what the work has done.
local function pie(r, from, to, color)
	local path = b.path():move_to(7, 7)
	local steps = math.max(math.ceil((to - from) / 12), 1)
	for step = 0, steps do
		local angle = math.rad(from + (to - from) * step / steps)
		path:line_to(7 + r * math.sin(angle), 7 - r * math.cos(angle))
	end
	return path:close():fill(color)
end

-- A ring drawn as short strokes with room between them, for a state not yet planned.
local function dashed(color)
	local paths = {}
	for dash = 0, 8 do
		local from = dash * 40
		local path = b.path()
		for step = 0, 3 do
			local angle = math.rad(from + step * 5.5)
			local x, y = 7 + 5.25 * math.sin(angle), 7 - 5.25 * math.cos(angle)
			if step == 0 then
				path:move_to(x, y)
			else
				path:line_to(x, y)
			end
		end
		paths[#paths + 1] = path:stroke(1.5, color)
	end
	return paths
end

local function canvas(paths)
	return b.canvas(paths):size(SIDE, SIDE)
end

-- The circle of a state. `kind` is the kind of the state (`started`, `unstarted`, `backlog`, `triage`,
-- `completed`, `canceled`, `other`); `late` is a started state after the first of its kind (a review
-- is further along than the work), which fills three quarters in the colour of what is done well.
function icons.state(kind, late)
	local paths = {}
	if kind == "backlog" or kind == "triage" then
		paths = dashed("dim")
	elseif kind == "unstarted" then
		paths[1] = b.path():circle(7, 7, 5.25):stroke(1.5, b.alpha("foreground", 0.75))
	elseif kind == "started" then
		local color = late and "success" or "warning"
		paths[1] = b.path():circle(7, 7, 5.25):stroke(1.5, color)
		paths[2] = pie(3.1, 0, late and 270 or 180, color)
	elseif kind == "completed" then
		paths[1] = b.path():circle(7, 7, 6):fill("accent")
		paths[2] = b.path():move_to(4.4, 7.2):line_to(6.2, 9):line_to(9.6, 5.2):stroke(1.5, "background")
	elseif kind == "canceled" then
		paths[1] = b.path():circle(7, 7, 6):fill("dim")
		paths[2] = b.path():move_to(5, 5):line_to(9, 9):move_to(9, 5):line_to(5, 9):stroke(1.5, "background")
	else
		paths[1] = b.path():circle(7, 7, 5.25):stroke(1.5, "dim")
	end
	return canvas(paths):no_shrink()
end

local BARS = { { 2, 4 }, { 5.75, 7 }, { 9.5, 10 } }
local RISING = { low = 1, medium = 2, high = 3 }

-- A priority: bars that rise, as many lit as it is high; an urgent one is a filled square with a
-- mark, and none is three quiet dots.
function icons.priority(name)
	local paths = {}
	if name == "urgent" then
		paths[1] = b.path()
			:move_to(4.5, 1.5)
			:line_to(9.5, 1.5)
			:quad_to(12.5, 1.5, 12.5, 4.5)
			:line_to(12.5, 9.5)
			:quad_to(12.5, 12.5, 9.5, 12.5)
			:line_to(4.5, 12.5)
			:quad_to(1.5, 12.5, 1.5, 9.5)
			:line_to(1.5, 4.5)
			:quad_to(1.5, 1.5, 4.5, 1.5)
			:close()
			:fill("warning")
		paths[2] = b.path():rect(6.2, 3.8, 1.6, 4.2):fill("background")
		paths[3] = b.path():rect(6.2, 9, 1.6, 1.6):fill("background")
	elseif RISING[name] == nil then
		for index, x in { 2, 5.75, 9.5 } do
			paths[index] = b.path():rect(x, 6.25, 2.5, 1.5):fill(b.alpha("foreground", 0.35))
		end
	else
		for index, bar in BARS do
			local color = b.alpha("foreground", index <= RISING[name] and 0.85 or 0.25)
			paths[index] = b.path():rect(bar[1], 12 - bar[2], 2.5, bar[2]):fill(color)
		end
	end
	return canvas(paths):no_shrink()
end

-- The arrow before a group's name: down while it is open, to the right while it is folded.
function icons.arrow(closed, color)
	local path = b.path()
	if closed then
		path:move_to(2.5, 1):line_to(2.5, 8):line_to(7, 4.5):close()
	else
		path:move_to(1, 2.5):line_to(8, 2.5):line_to(4.5, 7):close()
	end
	return b.canvas({ path:fill(color or "dim") }):size(9, 9):no_shrink()
end

-- Someone: a head over shoulders, for the assignee.
function icons.person()
	local color = b.alpha("foreground", 0.75)
	return canvas({
		b.path():circle(7, 5, 2.3):stroke(1.4, color),
		b.path():move_to(2.8, 12):cubic_to(2.8, 6.6, 11.2, 6.6, 11.2, 12):stroke(1.4, color),
	}):no_shrink()
end

-- The head of a group of the inbox: a dot for the unread, a ring for the read.
function icons.inbox(unread)
	if unread then
		return canvas({ b.path():circle(7, 7, 3.5):fill("accent") }):no_shrink()
	end
	return canvas({ b.path():circle(7, 7, 3.5):stroke(1.4, b.alpha("foreground", 0.45)) }):no_shrink()
end

-- A pull request: two commits on a line, and the line of the branch that comes back to it, its arrow
-- at the end (Tabler's git-pull-request, drawn at this size).
function icons.pull_request(color)
	color = color or b.alpha("foreground", 0.75)
	return canvas({
		b.path():circle(3.5, 3.5, 1.2):stroke(1.3, color),
		b.path():circle(3.5, 10.5, 1.2):stroke(1.3, color),
		b.path():circle(10.5, 10.5, 1.2):stroke(1.3, color),
		b.path():move_to(3.5, 4.7):line_to(3.5, 9.3):stroke(1.3, color),
		b.path():move_to(6.4, 3.5):line_to(9.3, 3.5):cubic_to(10, 3.5, 10.5, 4, 10.5, 4.7):line_to(10.5, 9.3):stroke(1.3, color),
		b.path():move_to(8.2, 5.2):line_to(6.4, 3.5):line_to(8.2, 1.8):stroke(1.3, color),
	}):no_shrink()
end

-- A box seen from a corner, for the project.
function icons.cube()
	local color = b.alpha("foreground", 0.75)
	return canvas({
		b.path()
			:move_to(7, 1.8)
			:line_to(11.6, 4.4)
			:line_to(11.6, 9.6)
			:line_to(7, 12.2)
			:line_to(2.4, 9.6)
			:line_to(2.4, 4.4)
			:close()
			:stroke(1.3, color),
		b.path():move_to(2.6, 4.5):line_to(7, 7):line_to(11.4, 4.5):move_to(7, 7):line_to(7, 12):stroke(1.3, color),
	}):no_shrink()
end

-- A clock, for the cycle.
function icons.cycle()
	local color = b.alpha("foreground", 0.75)
	return canvas({
		b.path():circle(7, 7, 5):stroke(1.3, color),
		b.path():move_to(7, 2):line_to(7, 7):line_to(10, 9):stroke(1.3, color),
	}):no_shrink()
end

-- Steps that rise, for the estimate.
function icons.estimate()
	local color = b.alpha("foreground", 0.75)
	return canvas({
		b.path():move_to(2, 11):line_to(5, 11):line_to(5, 8):line_to(8, 8):line_to(8, 5):line_to(12, 5):stroke(1.3, color),
	}):no_shrink()
end

-- A calendar, for the due date.
function icons.date()
	local color = b.alpha("foreground", 0.75)
	return canvas({
		b.path()
			:move_to(4, 3)
			:line_to(10, 3)
			:quad_to(12, 3, 12, 5)
			:line_to(12, 10)
			:quad_to(12, 12, 10, 12)
			:line_to(4, 12)
			:quad_to(2, 12, 2, 10)
			:line_to(2, 5)
			:quad_to(2, 3, 4, 3)
			:close()
			:stroke(1.3, color),
		b.path():move_to(2, 6):line_to(12, 6):move_to(5, 1.8):line_to(5, 4.2):move_to(9, 1.8):line_to(9, 4.2):stroke(1.3, color),
	}):no_shrink()
end

-- A tag, for the labels.
function icons.tag()
	local color = b.alpha("foreground", 0.75)
	return canvas({
		b.path():move_to(2, 2.6):line_to(6.6, 2.6):line_to(11.8, 7.8):line_to(7.6, 12):line_to(2, 6.4):close():stroke(1.3, color),
		b.path():circle(4.8, 5.2, 0.9):fill(color),
	}):no_shrink()
end

-- The dot of a label, in the colour Linear gave it: data, not a role of the theme.
function icons.dot(color)
	return b.div():size(7, 7):rounded(4):bg(color or "dim"):no_shrink()
end

return icons
