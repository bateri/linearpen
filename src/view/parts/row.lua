-- The two kinds of line in the list: the head of a group and an issue. They are only drawn here;
-- what a press on one does is in list.lua.

local kit = require("../lib/kit")
local icons = require("./icons")

local b, u, SIZE = kit.b, kit.u, kit.SIZE

local row = {}

-- The name a line answers the pointer by.
function row.name(key)
	return "row:" .. key
end

-- What stands on the line's ground while the pointer is over it, and what the chosen line is.
local function ground(look)
	if look.selected then
		return b.alpha("accent", 0.37), b.alpha("accent", 0.7)
	elseif look.marked then
		return b.alpha("accent", 0.14), b.alpha("foreground", 0)
	elseif look.hovered then
		return b.alpha("foreground", 0.07), b.alpha("foreground", 0)
	end
	return nil, b.alpha("foreground", 0)
end

-- A line of the list, inset from both sides, with a thin frame that is only seen when chosen.
local function line(name, look)
	local fill, frame = ground(look)
	return b.div()
		:press(name)
		:role("menu_item")
		:row()
		:items_center()
		:h(SIZE.row)
		:w_full()
		:px(9)
		:rounded(7)
		:bg(fill)
		:border(1, frame)
		:motion({ seconds = u.theme.motion, color = true })
end

-- The head of a group: its arrow, the circle of its state, its name and how many it holds. It is
-- as tall as an issue, with four points above it that set it apart from the group before.
function row.group(item, look, late)
	local group = item.group
	local inner = line(row.name(item.key), look)
		:gap(10)
		:label(string.format("%s, %d", group.name, group.count))
		:selected(look.selected)
		:expanded(not group.closed)
	inner:child(icons.arrow(group.closed))
	inner:child(icons.state(group.kind, late))
	inner:child(b.text(group.name):weight(700):color(group.closed and "dim" or "foreground"):no_shrink())
	inner:child(b.text(tostring(group.count)):text_size(SIZE.small):color("dim"):no_shrink())
	return b.div():h(SIZE.row + SIZE.group_gap):pt(SIZE.group_gap):px(9):no_shrink():child(inner)
end

-- A label of an issue: the dot in Linear's colour and the name.
local function label(info)
	return b.div():row():items_center():gap(6):no_shrink():child(icons.dot(info.color)):child(
		b.text(info.name):text_size(SIZE.tiny):color("dim"):no_shrink()
	)
end

-- The name the box of an issue's selection answers the pointer by.
function row.check_name(key)
	return "check:" .. key
end

-- The box of a selection: seen on the line the cursor is on, and on every line once one is selected;
-- its place is kept where it is not seen, so that no line moves.
local function check(key, look)
	local slot = b.div():size(14, 14):no_shrink()
	if not look.check then
		return slot
	end
	local name = row.check_name(key)
	slot:press(name):role("checkbox"):checked(look.marked):label("Select")
	local box = b.div():size(14, 14):rounded(4):no_shrink()
	if look.marked then
		box:bg("accent"):child(b.canvas({
			b.path():move_to(3.6, 7.3):line_to(5.9, 9.6):line_to(10.4, 4.6):stroke(1.7, "background"),
		}):size(14, 14))
	else
		box:border(1, b.alpha("foreground", u:hovered(name) and 0.8 or 0.45))
	end
	return slot:child(box)
end

-- An issue: the box of a selection, its priority, its identifier, its title that gives way, its
-- labels, whose it is (in the list of all issues, where the page is wide enough) and the day it
-- changed. `o`: `labels` and `who` say whether there is room for them, `scope` the list shown.
function row.issue(item, look, o)
	local issue = item.row
	local el = line(row.name(item.key), look):gap(10):label(issue.identifier .. ", " .. issue.title):selected(look.selected)
	el:child(check(item.key, look))
	el:child(icons.priority(issue.priority))
	el:child(
		b.div():w(SIZE.identifier):no_shrink():child(b.text(issue.identifier):text_size(SIZE.small):color("dim"):truncate())
	)
	el:child(b.text(issue.title):grow(1):min_w(0):truncate())
	if o.labels then
		for index, info in issue.labels do
			if index > 3 then
				break
			end
			el:child(label(info))
		end
	end
	if o.scope == "all" and o.who then
		el:child(
			b.div():w(SIZE.who):no_shrink():row():justify_end():child(
				b.text(issue.assignee or ""):text_size(SIZE.tiny):color("dim"):truncate()
			)
		)
	end
	el:child(
		b.div():w(SIZE.date):no_shrink():row():justify_end():child(
			b.text(issue.updated):text_size(SIZE.small):color("dim"):no_shrink()
		)
	)
	return b.div():h(SIZE.row):px(9):no_shrink():child(el)
end

return row
