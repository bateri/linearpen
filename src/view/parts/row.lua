-- The kinds of line in the list: the head of a group, an issue and a project. They are only drawn here;
-- what a press on one does is in list.lua.

local kit = require("../lib/kit")
local icons = require("./icons")

local b, u, SIZE, act = kit.b, kit.u, kit.SIZE, kit.act

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

-- The arrow of a group's head, which points at its lines where they are open and turns to them as the
-- group opens (and back as it folds), where the terminal's kit turns what it draws.
local function arrow(key, closed)
	local drawn = icons.arrow(true)
	if not kit.has("element.transform") or drawn.transform == nil then
		return icons.arrow(closed)
	end
	return drawn
		:id("arrow:" .. key)
		:transform({ rotate = closed and 0 or 90 })
		:motion({ transform = true, seconds = 0.24 })
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
	inner:child(arrow(item.key, group.closed))
	if group.kind == "unread" or group.kind == "read" then
		inner:child(icons.inbox(group.kind == "unread"))
	else
		inner:child(icons.state(group.kind, late))
	end
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

-- The box of a selection as it is drawn, named `name` for the pointer: ticked where `marked`.
function row.box(name, marked)
	local box = b.div():size(14, 14):rounded(4):no_shrink()
	if marked then
		box:bg("accent"):child(b.canvas({
			b.path():move_to(3.6, 7.3):line_to(5.9, 9.6):line_to(10.4, 4.6):stroke(1.7, "background"),
		}):size(14, 14))
	else
		box:border(1, b.alpha("foreground", u:hovered(name) and 0.8 or 0.45))
	end
	return box
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
	return slot:child(row.box(name, look.marked))
end

-- Whether `why`, the error of `carry`, is its refusal of what it was given (the kit's rule of what a
-- drag out of the view may carry), and not a mistake in the call.
local function refused(why)
	return type(why) == "string"
		and (string.find(why, "a carry is one line", 1, true) ~= nil or string.find(why, "the url of a carry", 1, true) ~= nil)
end

-- A title as one line: what ends a line, moves a terminal or is no letter (the controls, the line and
-- paragraph separators, the byte order mark) is a space, as the pasteboard's copy of it is (`context.lua`).
-- What sets the direction of text (the embeddings, overrides and isolates) is left, for the kit to refuse.
local function one_line(title)
	title = string.gsub(title, "%c", " ")
	title = string.gsub(title, "\xC2[\x80-\x9F]", " ")
	title = string.gsub(title, "\xE2\x80[\xA8\xA9]", " ")
	title = string.gsub(title, "\xEF\xBB\xBF", " ")
	return (string.gsub(title, " +", " "))
end

-- The issue as words on one line: its identifier and its title. What a line dragged out of the page
-- carries, and what Share copies, before the address.
function row.words(issue)
	return issue.identifier .. " " .. one_line(issue.title)
end

-- What the line brings when it is dragged out of the page to a terminal pane or another application:
-- the issue as one line of words (its identifier, its title and its address) and the address for the
-- applications that take one. The title is Linear's, written by anyone: one with a control of the
-- direction of text in it, which the kit may not hand the system (`carry` refuses it by an error),
-- leaves the line a line of the page alone, and an address it may not hand on leaves the words without
-- it. A terminal whose kit has no `carry` leaves the line as it is; any other error is the page's own.
local function carries(el, issue)
	if el.carry == nil then
		return
	end
	local words = row.words(issue)
	local url = issue.url
	if url ~= nil and url ~= "" then
		local ok, why = pcall(el.carry, el, { text = words .. " " .. url, url = url })
		if ok then
			return
		elseif not refused(why) then
			error(why, 0)
		end
	end
	local ok, why = pcall(el.carry, el, { text = words })
	if not ok and not refused(why) then
		error(why, 0)
	end
end

-- The name the agent's mark on the line of the issue `id` answers the pointer by.
function row.agent_name(id)
	return "agent:" .. id
end

-- The name of the dot that says the agent of the issue `id` has stopped.
function row.idle_name(id)
	return "agent-idle:" .. id
end

-- The agent that works on an issue, after its title: a button that says where it goes, `Go to Claude`,
-- with the ring while the agent's pane writes and a dot of warning where it has written nothing for a
-- while (the agent is done, or it waits for the person: which of them cannot be told, so the tip says
-- both). The ring and the dot are as wide and the words stay, so the line does not move when the one
-- gives way to the other. A press on it takes the keyboard to the agent's pane.
local function agent(id, working)
	local t = u.theme
	local name = row.agent_name(id)
	local over = u:hovered(name)
	local held = over and u:pressed(name)
	local go = "Go to " .. working.agent
	local mark = b.div()
		:press(name)
		:role("button")
		:row()
		:items_center()
		:gap(6)
		:h(SIZE.bar_control + 4)
		:px(7)
		:no_shrink()
		:rounded(t.radius.control)
		:bg(held and t.control_press or over and t.control_hover or t.control)
		:border(1, b.alpha("foreground", over and 0.22 or 0.1))
		:motion({ seconds = t.motion, color = true })
	if working.idle then
		-- (The state is in the label too, for what reads the page aloud.) The dot breathes, slowly, where
		-- the terminal's kit loops: the agent may wait for the person.
		local stopped = working.agent .. " has stopped: it is done, or it waits for you"
		local dot = b.div():size(7, 7):rounded(4):bg("warning")
		if kit.has("motion.loop") then
			dot:motion({ loop = { seconds = 1.6, opacity = 0.35 } })
		end
		return mark:label(go .. ", " .. stopped):tip(stopped .. ". Go to its pane"):child(
			b.div():id(row.idle_name(id)):size(12, 12):items_center():justify_center():no_shrink():child(dot)
		):child(b.text(go):text_size(SIZE.small):color("warning"):no_shrink())
	end
	return mark:label(go .. ", at work"):tip(working.agent .. " works on it. Go to its pane"):child(
		kit.spinner(12, "accent")
	):child(b.text(go):text_size(SIZE.small):color("accent"):no_shrink())
end

-- An issue: the box of a selection, its priority, its identifier, its title that gives way, the
-- agent that is on it, its labels, whose it is (in any list but the person's own, where the page is wide
-- enough) and the day it changed. `o`: `labels` and `who` say whether there is room for them, `scope` the list shown.
-- What a row of the inbox says of its notification, in words: who and what (`Deniz · Mention`).
local function note_words(note)
	if note.who ~= nil and note.who ~= "" then
		return note.who .. " \u{b7} " .. note.what
	end
	return note.what
end

function row.issue(item, look, o)
	local issue = item.row
	local note = issue.note
	local spoken = issue.identifier .. ", " .. issue.title
	if note ~= nil then
		-- (Whether it is unread is its group's, said once.)
		spoken = spoken .. ", " .. note_words(note)
	end
	local el = line(row.name(item.key), look):gap(10):label(spoken):selected(look.selected)
	if note ~= nil and act.marking(issue.id) then
		-- The line being marked read leaves the unread fading (only its opacity changes): no other line
		-- that leaves the list (a fold, the filter, another list) does.
		el:motion({ seconds = u.theme.motion, color = true, exit = { opacity = 0, seconds = 0.24 } })
	end
	carries(el, issue)
	el:child(check(item.key, look))
	el:child(icons.priority(issue.priority))
	el:child(
		b.div():w(SIZE.identifier):no_shrink():child(b.text(issue.identifier):text_size(SIZE.small):color("dim"):truncate())
	)
	el:child(b.text(issue.title):grow(1):min_w(0):truncate())
	if look.working ~= nil then
		el:child(agent(issue.id, look.working))
	end
	if o.labels and note == nil then
		for index, info in issue.labels do
			if index > 3 then
				break
			end
			el:child(label(info))
		end
	end
	if note ~= nil then
		el:child(
			b.div():w(o.noted and SIZE.note or SIZE.note_short):no_shrink():row():justify_end():child(
				b.text(o.noted and note_words(note) or note.what)
					:text_size(SIZE.small)
					:color(note.unread and "foreground" or "dim")
					:truncate()
			)
		)
	end
	if note == nil and o.scope ~= "mine" and o.who then
		el:child(
			b.div():w(SIZE.who):no_shrink():row():justify_end():child(
				b.text(issue.assignee or ""):text_size(SIZE.tiny):color("dim"):truncate()
			)
		)
	end
	if look.changing then
		-- A change of it is on its way.
		el:child(b.div():row():items_center():no_shrink():label("Changing"):child(kit.spinner(12, "dim")))
	end
	el:child(
		b.div():w(SIZE.date):no_shrink():row():justify_end():child(
			b.text(issue.updated):text_size(SIZE.small):color("dim"):no_shrink()
		)
	)
	return b.div():h(SIZE.row):px(9):no_shrink():child(el)
end

-- What a line of the list of projects says aloud: its name, its health, how far it is, and its target.
local function project_words(project)
	local said = icons.HEALTH[project.health]
	local words = project.name .. ", " .. (said ~= nil and said.word or "No update")
	if said ~= nil and project.stale then
		words = words .. ", outdated"
	end
	words = words .. string.format(", %d%%", project.progress)
	if project.target ~= nil then
		words = words .. ", target " .. project.target .. (project.late and ", late" or "")
	end
	return words
end

-- A project: its health (by shape and colour), its name that gives way, the word of its health and its
-- lead where the list has room, the day it is for (in the colour of an error where that has passed) and
-- how far it is. Every column keeps its width, so the lines are a table and nothing moves as they change.
function row.project(item, look, o)
	local project = item.row
	local el = line(row.name(item.key), look):gap(10):label(project_words(project)):selected(look.selected)
	el:child(icons.health(project.health, project.stale))
	el:child(b.text(project.name):grow(1):min_w(0):truncate())
	if o.labels then
		local said = icons.HEALTH[project.health]
		el:child(
			b.div():w(SIZE.health):no_shrink():child(
				b.text(said ~= nil and said.word or "No update")
					:text_size(SIZE.small)
					:color((said ~= nil and not project.stale) and said.color or "dim")
					:truncate()
			)
		)
	end
	if o.who then
		el:child(
			b.div():w(SIZE.who):no_shrink():row():justify_end():child(
				b.text(project.lead or ""):text_size(SIZE.tiny):color("dim"):truncate()
			)
		)
	end
	el:child(
		b.div():w(SIZE.target):no_shrink():row():justify_end():child(
			b.text(project.target or ""):text_size(SIZE.small):color(project.late and "error" or "dim"):truncate()
		)
	)
	el:child(
		b.div():w(SIZE.progress):no_shrink():row():items_center():justify_end():gap(5):child(icons.progress(project.progress)):child(
			b.text(tostring(project.progress) .. "%"):text_size(SIZE.small):color("dim"):no_shrink()
		)
	)
	return b.div():h(SIZE.row):px(9):no_shrink():child(el)
end

return row
