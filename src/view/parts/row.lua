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

-- An issue: room for the box of a selection (the next phase), its priority, its identifier, its
-- title that gives way, its labels, whose it is (in the list of all issues, where the page is wide
-- enough) and the day it changed.
function row.issue(item, look, width, scope)
	local issue = item.row
	local el = line(row.name(item.key), look):gap(10):label(issue.identifier .. ", " .. issue.title):selected(look.selected)
	el:child(b.div():size(14, 14):no_shrink())
	el:child(icons.priority(issue.priority))
	el:child(
		b.div():w(SIZE.identifier):no_shrink():child(b.text(issue.identifier):text_size(SIZE.small):color("dim"):truncate())
	)
	el:child(b.text(issue.title):grow(1):min_w(0):truncate())
	if width >= SIZE.labels then
		for index, info in issue.labels do
			if index > 3 then
				break
			end
			el:child(label(info))
		end
	end
	if scope == "all" and width >= SIZE.assignee then
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
