-- A project of the list of projects, beside the list as an issue is: Show issues, Open in Linear and
-- Close over it; its state, its name, its health, how far it is, its lead and its days; then its last
-- update (whole, as Linear has it) and its milestones. What its line told is drawn at once; the words of
-- the update and the milestones are asked for (`project`) and come into their place under the rest, so
-- nothing over them moves when they come. Read only: nothing of a project is changed here.

local kit = require("../lib/kit")
local markdown = require("@pencere/markdown")
local commands = require("./commands")
local icons = require("./icons")
local list = require("./list")
local lists = require("./lists")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local project = {}

-- The box that scrolls (Space and Shift Space, and the arrows where the project has the pane).
local SCROLL = "project-scroll"

-- The width of the names of the facts (`Health`, `Progress`), and of a milestone's percent and day.
local LABEL = 72
local PERCENT = 40
local DAY = 64

-- `open`: the project shown (`{ id, row, state }`, its line as the list has it and its group's state);
-- `kept`: what Linear told of each project asked for (`{ status, data, error, serial }`), until the
-- lists are asked for again; `back`: the project whose issues were gone to, to come back to with Esc;
-- `returning`: the project come back to, put beside the projects when they are told;
-- `trees`: the update's words, read as markdown, by their text.
local J = { open = nil, serial = 0, kept = {}, back = nil, returning = nil, trees = {} }

-- Asks Linear (`project`) for the update and the milestones of the project `id`. An answer that a later
-- asking went past is let go of.
local function ask(id)
	J.serial += 1
	local serial = J.serial
	J.kept[id] = { status = "loading", serial = serial }
	kit.call("project", { id = id }, function(found, err)
		local kept = J.kept[id]
		if kept == nil or kept.serial ~= serial then
			return true
		end
		if err then
			kept.status, kept.error = "failed", err.message or err.code
		else
			kept.status, kept.data = "ready", found
		end
		return true
	end)
end

function project.is_open()
	return J.open ~= nil
end

-- The id of the project shown, or nil.
function project.id()
	return J.open and J.open.id or nil
end

-- Shows the project of `item`, a line of the list of projects. Another project is read from its top.
-- Whether it was one.
function project.show(item)
	if item == nil or item.kind ~= "project" then
		return false
	end
	local row = item.row
	if J.open ~= nil and J.open.id ~= row.id then
		kit.scroll_to(SCROLL, 0)
	end
	J.open = { id = row.id, row = row, state = { name = item.group.name, kind = item.group.kind } }
	local kept = J.kept[row.id]
	if kept == nil or kept.status == "failed" then
		ask(row.id)
	end
	return true
end

function project.close()
	J.open = nil
end

-- The project shown takes what the list tells of it now (its line is made again at each push).
function project.sync()
	if J.open == nil then
		return
	end
	local item = list.item_of("p:" .. J.open.id)
	if item ~= nil then
		J.open.row = item.row
		J.open.state = { name = item.group.name, kind = item.group.kind }
	end
end

-- The lists are asked for again: what was told of each project is let go of, and the one shown is
-- asked for again.
function project.forget()
	table.clear(J.kept)
	if J.open ~= nil then
		ask(J.open.id)
	end
end

act.lists_asked = project.forget

-- Asks again for the project shown, where Linear did not tell it. The reply.
function project.retry()
	if J.open ~= nil then
		ask(J.open.id)
	end
	return true
end

-- The project the keys are for: the one shown, else the one under the cursor. Its line, or nil.
local function target()
	if J.open ~= nil then
		return J.open.row
	end
	local item = list.current()
	return item ~= nil and item.kind == "project" and item.row or nil
end

-- Whether there is a project the keys are for (Show issues, Open in Linear).
function project.targeted()
	return S.scope == "projects" and target() ~= nil
end

-- Opens the project in Linear, in the person's browser. The reply, or nil.
function project.link()
	local row = target()
	if row == nil or row.url == nil or row.url == "" then
		return nil
	end
	return { link = row.url }
end

-- Goes to the issues of the project the keys are for, as a list of their own; Esc there comes back to
-- the list of projects, on it. The words of the filter were the projects' and stay with them. The reply.
function project.show_issues()
	local row = target()
	if row == nil then
		return true
	end
	lists.remember("project", row.id, row.name)
	J.back = { id = row.id }
	J.open = nil
	if act.field_clear ~= nil then
		act.field_clear()
	end
	act.view({ list = { kind = "project", id = row.id }, filter = "" })
	return true
end

-- Another list is gone to than the issues of the project gone to from the projects: Esc no longer comes
-- back to them.
act.list_going = function(going)
	if J.back ~= nil and not (going.kind == "project" and going.id == J.back.id) then
		J.back = nil
	end
	if going.kind ~= "projects" then
		J.returning = nil
	end
end

-- Whether the list shown is the issues of a project gone to from the list of projects.
function project.can_go_back()
	return J.back ~= nil and S.scope == "project" and S.list_id == J.back.id
end

-- Back to the list of projects, the cursor on the project whose issues were shown, and it beside the
-- list again. The reply.
function project.go_back()
	J.returning = J.back.id
	J.back = nil
	act.view({ list = { kind = "projects" } })
	return true
end

-- The lists are told: the project come back to has the cursor and stands beside them, and the one
-- shown takes what its line says now.
function project.listed()
	-- (The projects are asked for as they are shown: the first answer says they are on their way.)
	local listing = S.issues ~= nil and S.issues.listing or nil
	if J.returning ~= nil and S.scope == "projects" and listing ~= "loading" then
		local key = "p:" .. J.returning
		J.returning = nil
		if list.go_to(key) then
			project.show(list.item_of(key))
		end
	end
	project.sync()
end

-- Scrolls the project by a view (`way` 1 or -1). Whether there is one.
function project.scroll(way)
	if J.open == nil then
		return false
	end
	local extent = b.layout.scroll_extent(SCROLL)
	local room = extent ~= nil and extent.view or 0
	kit.scroll_to(SCROLL, (kit.offset(SCROLL) or 0) + way * math.max(room - 48, 48), true)
	return true
end

-- Scrolls the project by `points`, or to its `top` or its `end`.
function project.scroll_by(points, to)
	if J.open == nil then
		return false
	end
	local extent = b.layout.scroll_extent(SCROLL)
	local last = extent ~= nil and extent.max_offset or 0
	local y = (kit.offset(SCROLL) or 0) + (points or 0)
	if to == "top" then
		y = 0
	elseif to == "end" then
		y = last
	end
	kit.scroll_to(SCROLL, math.min(math.max(y, 0), last), true)
	return true
end

-- --- Drawing ------------------------------------------------------------------------------------------

-- The head of a section, as the issue's sections are named (`MILESTONES 3`).
local function heading(name, count)
	local row = b.div():row():items_center():gap(8):mt(20):mb(6):no_shrink()
	row:child(b.text(name):text_size(SIZE.tiny):color("dim"))
	if count ~= nil then
		row:child(b.text(count):text_size(SIZE.tiny):color("dim"))
	end
	return row
end

-- A fact: its name in a column of its own, and what it is.
local function fact(name, value)
	return b.div():row():items_center():gap(8):h(26):no_shrink():child(
		b.div():w(LABEL):no_shrink():child(b.text(name):text_size(SIZE.small):color("dim"))
	):child(value)
end

-- The health, by its shape, its word and its colour together, in a pill; one Linear holds outdated is
-- quieter, and says so. (Its colour fades into the next project's as the cursor goes.)
local function health(row)
	local said = icons.HEALTH[row.health]
	local box = b.div():row():items_center():gap(8):min_w(0)
	if said == nil then
		return box:child(icons.health(nil)):child(b.text("No update"):text_size(SIZE.small):color("dim"))
	end
	local color = row.stale and "dim" or said.color
	box:child(
		b.div()
			:id("project-health")
			:row()
			:items_center()
			:gap(6)
			:h(22)
			:px(9)
			:rounded(11)
			:no_shrink()
			:bg(b.alpha(color, 0.14))
			:border(1, b.alpha(color, 0.45))
			:motion({ seconds = u.theme.motion, color = true })
			:child(icons.health(row.health, row.stale))
			:child(b.text(said.word):text_size(SIZE.small):weight(700):color(color))
	)
	if row.stale then
		box:child(b.text("Outdated"):text_size(SIZE.small):color("dim"):truncate():min_w(0))
	end
	return box
end

-- How far it is: a bar as wide as `room`, filled as far as it is (the fill glides to the next project's
-- as the cursor goes), and the percent after it.
local function progress(row, room)
	local part = math.min(math.max(row.progress, 0), 100) / 100
	local bar = math.max(room - PERCENT - 8, 40)
	local done = row.progress >= 100
	return b.div():row():items_center():gap(8):child(
		b.div():w(bar):h(6):rounded(3):no_shrink():bg(b.alpha("foreground", 0.12)):child(
			b.div()
				:id("project-progress")
				:h(6)
				:w(math.floor(bar * part + 0.5))
				:rounded(3)
				:bg(done and "success" or "accent")
				:motion({ seconds = 0.24, size = true, color = true })
		)
	):child(b.text(tostring(row.progress) .. "%"):text_size(SIZE.small):color("dim"):no_shrink())
end

-- Its days: when it started and the day it is for, that one in the colour of an error where it has passed.
local function days(row)
	local box = b.div():row():items_center():gap(6):min_w(0)
	if row.start == nil and row.target == nil then
		return box:child(b.text("No dates"):text_size(SIZE.small):color("dim"))
	end
	if row.start ~= nil then
		box:child(b.text(row.start):text_size(SIZE.small):no_shrink())
	end
	if row.target ~= nil then
		box:child(b.text("\u{2192}"):text_size(SIZE.small):color("dim"):no_shrink())
		box:child(b.text(row.target):text_size(SIZE.small):color(row.late and "error" or "foreground"):no_shrink())
		if row.late then
			box:child(b.text("Late"):text_size(SIZE.small):weight(700):color("error"):no_shrink())
		end
	end
	return box
end

-- A picture in the words of an update: not drawn here (it is Linear's, of the update), a line that says
-- so in its place.
local function picture(_, alt)
	local named = (alt ~= nil and alt ~= "") and alt or "A picture"
	return b.div():row():items_center():gap(8):h(26):px(9):rounded(7):no_shrink():bg(b.alpha("foreground", 0.06)):child(
		b.text("Image"):text_size(SIZE.small):weight(700):no_shrink()
	):child(b.text(named):text_size(SIZE.small):color("dim"):truncate():min_w(0)):child(b.spacer()):child(
		b.text("In Linear"):text_size(SIZE.small):color("dim"):no_shrink()
	)
end

-- The words of the update, markdown written by someone else, drawn as text. Words that are not valid
-- markdown are shown as they came.
local function words(text, id)
	local tree = J.trees[text]
	if tree == nil then
		local ok, parsed = pcall(markdown.parse, text)
		tree = ok and parsed or false
		J.trees[text] = tree
	end
	if tree == false then
		return b.text(text):text_size(SIZE.small):wrap()
	end
	return markdown.render(tree, { id = id, gap = 8, image = picture })
end

-- A milestone: how far it is (the ring of the list's lines), its name, its percent and its day.
local function milestone(one)
	return b.div():row():items_center():gap(8):h(24):no_shrink():label(
		string.format("%s, %d%%%s", one.name, one.progress, one.target ~= nil and (", due " .. one.target) or "")
	):child(icons.progress(one.progress)):child(
		b.text(one.name):text_size(SIZE.small):color(one.progress >= 100 and "dim" or "foreground"):truncate():min_w(0):grow(1)
	):child(
		b.div():w(PERCENT):no_shrink():row():justify_end():child(
			b.text(tostring(one.progress) .. "%"):text_size(SIZE.small):color("dim")
		)
	):child(
		b.div():w(DAY):no_shrink():row():justify_end():child(b.text(one.target or ""):text_size(SIZE.small):color("dim"))
	)
end

-- The last update: who posted it and when, at once; its words and the milestones when Linear tells them.
local function body(open)
	local row = open.row
	local box = b.div():col():no_shrink()
	local head = heading("LATEST UPDATE")
	if row.update ~= nil then
		head:child(b.spacer())
		head:child(b.text((row.update.who or "Someone") .. " \u{b7} " .. row.update.at):text_size(SIZE.tiny):color("dim"))
	end
	box:child(head)
	local kept = J.kept[open.id]
	if kept == nil or kept.status == "loading" then
		-- (What the line told of the update is shown while its words are asked for.)
		if row.update ~= nil and row.update.summary ~= nil then
			box:child(b.text(row.update.summary):text_size(SIZE.small):color("dim"):wrap():no_shrink())
		end
		return box:child(
			b.div():row():items_center():gap(8):mt(12):no_shrink():child(kit.spinner(13, "dim")):child(
				b.text("Asking Linear"):text_size(SIZE.small):color("dim")
			)
		)
	elseif kept.status == "failed" then
		return box:child(
			b.div():col():gap(10):mt(4):items_start():no_shrink():child(
				b.div():row():items_start():gap(8):child(b.div():size(7, 7):rounded(4):bg("warning"):no_shrink():mt(6)):child(
					b.text(kept.error or "Linear did not tell it"):text_size(SIZE.small):color("warning"):wrap():min_w(0)
				)
			):child(kit.button("project-retry", "Try again"))
		)
	end
	local data = kept.data
	-- (It comes in as it comes, fading: what is over it stays where it was.)
	-- (One box for every project: going from a project told to the next does not bring it in again, so an
	-- arrow held down does not leave it half seen.)
	local told = b.div():id("project-body"):col():no_shrink():motion({ enter = { opacity = 0, seconds = 0.24 } })
	if data.update ~= nil then
		told:child(words(data.update, "project-update:" .. open.id))
	elseif row.update ~= nil and row.update.summary ~= nil then
		told:child(b.text(row.update.summary):text_size(SIZE.small):wrap())
	else
		told:child(b.text("No update yet"):text_size(SIZE.small):color("dim"))
	end
	told:child(heading("MILESTONES", #data.milestones > 0 and tostring(#data.milestones) or nil))
	if #data.milestones == 0 then
		told:child(b.text("No milestones"):text_size(SIZE.small):color("dim"))
	end
	for _, one in data.milestones do
		told:child(milestone(one))
	end
	return box:child(told)
end

-- The project shown, `width` points wide.
function project.view(width)
	local open = J.open
	local row = open.row
	local room = math.max(width - 2 * SIZE.pad - LABEL - 8, 80)
	local top = b.div():col():no_shrink():pt(SIZE.head_top):px(SIZE.pad):pb(14)
	-- Going to its issues at the start of the line (the project's own verb, as Work on is the issue's),
	-- leaving it at the end.
	top:child(
		b.div():row():items_center():gap(8):no_shrink():child(kit.button("project-issues", "Show issues", {
			primary = true,
			suffix = commands.label("show_issues"),
			tip = "Go to its issues: Esc comes back here",
		})):child(b.spacer()):child(kit.button("project-linear", "Open in Linear", {
			suffix = commands.label("link"),
			disabled = row.url == nil or row.url == "",
			tip = "Open the project in your browser",
		})):child(kit.button("project-close", "Close", { suffix = "Esc" }))
	)
	-- Its state, as the head of its group says it.
	top:child(
		b.div():row():items_center():gap(8):mt(9):h(22):no_shrink():child(icons.state(open.state.kind)):child(
			b.text(open.state.name):text_size(SIZE.small):color("dim"):truncate():min_w(0)
		)
	)
	top:child(b.text(row.name):text_size(16):weight(700):wrap():mt(3):mb(8):no_shrink())
	top:child(fact("Health", health(row)))
	top:child(fact("Progress", progress(row, room)))
	top:child(fact("Lead", b.text(row.lead or "No lead"):text_size(SIZE.small):color(row.lead and "foreground" or "dim"):truncate():min_w(0)))
	top:child(fact("Dates", days(row)))
	local content = b.div():col():grow(1):min_w(0):min_h(0):child(top)
	content:child(b.div():h(1):bg(b.alpha("foreground", 0.18)):no_shrink())
	content:child(b.div():id(SCROLL):grow(1):min_h(0):scroll():col():pt(2):px(SIZE.pad):pb(SIZE.pad):child(body(open)))
	return b.div():row():w(width):no_shrink():min_h(0):child(content)
end

return project
