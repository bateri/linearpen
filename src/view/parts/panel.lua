-- The issue beside the list: what a row does not carry (its description and its comments, asked of the
-- program when the issue is opened and kept until the lists come again), under the identifier, the
-- title and what the row knew.
-- The text of an issue is written by others: it is shown as text (markdown, drawn by the kit's
-- library), and a link in it opens only where the person presses it.

local markdown = require("@pencere/markdown")
local kit = require("../lib/kit")
local change = require("./change")
local comment = require("./comment")
local attach = require("./attach")
local edit = require("./edit")
local commands = require("./commands")
local icons = require("./icons")
local list = require("./list")
local pictures = require("./pictures")
local picker = require("./picker")
local values = require("./values")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local panel = {}

-- The issue open: what the row knew of it when it was opened (`{ id, identifier, title, priority, labels,
-- state }`: all it says where the list has the issue no more and the program has not told yet), and
-- what the program told of it, by id: `{ status = "loading" | "ready" | "failed", issue, error, at }`,
-- `at` being the lists it was asked under (their `updated`). The program lets go of the issues it kept
-- when Linear is asked for the lists again (Refresh, the five minutes, an issue made): an issue told
-- under older lists is asked for again when it is opened, and the issue open as the lists come; what is
-- shown stays until the new words come. `asked` numbers the questions of each issue and `told` is the
-- number of the one whose answer is shown: an answer older than it is not shown. `again` is the id of
-- the issue open while it is asked for again as the lists came from Linear (the line over the page,
-- lit while they were asked for, stays until it has come).
-- `pin` is set while a change of the issue is on its way or has just been made: the issue stays open
-- (it may have left the list) until the cursor is moved by the person (`{ id, moves }`). `near` is
-- the id of an issue the person came to from the one that was open, which the list does not hold.
-- `serial` numbers the menu of the neighbours (a pick of an older one is not one).
-- `room` is the points the text of an issue has across, as it was drawn last (the pictures of the text
-- are no wider).
local P = { open = nil, kept = {}, asked = {}, told = {}, again = nil, pin = nil, near = nil, serial = 0, room = 400 }
panel.state = P

-- The box that scrolls: the description and the comments.
local SCROLL = "panel-scroll"

-- The room an issue takes beside the list (the page is this wide), points: a part of the page, held
-- between two widths; a pane too narrow for both takes the whole of it.
function panel.width(page)
	if page < 760 then
		return page
	end
	-- (At least 420: the three buttons over an issue are whole in it.)
	return math.floor(math.min(520, math.max(420, page * 0.44)) + 0.5)
end

function panel.is_open()
	return P.open ~= nil
end

-- The id of the issue open, or nil.
function panel.id()
	return P.open and P.open.id or nil
end

-- What the program told of the open issue, when it has.
function panel.detail()
	local entry = P.open and P.kept[P.open.id]
	return entry ~= nil and entry.status == "ready" and entry.issue or nil
end

-- The lists the program told last (when they came): what is asked now is asked under them.
local function lists_at()
	return S.issues ~= nil and S.issues.updated or nil
end

-- A new question of the issue `id`: its number.
local function question(id)
	local n = (P.asked[id] or 0) + 1
	P.asked[id] = n
	return n
end

-- Whether the answer to the question `n` of the issue `id` is newer than what is shown of it: it is
-- then what is shown.
local function newer(id, n)
	if n <= (P.told[id] or 0) then
		return false
	end
	P.told[id] = n
	return true
end

-- Shows what the program told of the issue `id`.
local function keep(id, issue, at)
	P.kept[id] = { status = "ready", issue = issue, at = at }
	if P.open ~= nil and P.open.id == id then
		pictures.track(id, issue)
	end
end

local function ask(id)
	local n, at = question(id), lists_at()
	P.kept[id] = { status = "loading", at = at }
	kit.call("issue", { id = id }, function(result, err)
		if not newer(id, n) then
			return true
		end
		if err then
			P.kept[id] = { status = "failed", error = err.message or err.code, at = at }
		else
			keep(id, result, at)
		end
		return true
	end)
end

-- Asks again for the issue open, without a loading: what is shown stays until it comes (and where it
-- does not come). `done` is called when it has. Whether it asked (it does not where the issue is not
-- the one open).
function panel.reask(id, done)
	if P.open == nil or P.open.id ~= id then
		return false
	end
	local n, at = question(id), lists_at()
	kit.call("issue", { id = id }, function(result, err)
		if not err and newer(id, n) then
			keep(id, result, at)
		end
		done()
		return true
	end)
	return true
end

-- The lists came (`lit`: while the line over the page said Linear was asked for them). The issue open,
-- told under older lists, is asked for again, and the line stays until it has come where it was lit.
-- Where it does not come, what is shown stays, and the next lists ask again.
function panel.listed(lit)
	local id = P.open and P.open.id or nil
	local entry = id ~= nil and P.kept[id] or nil
	if entry == nil or entry.status == "loading" or entry.at == lists_at() then
		return
	end
	if entry.status == "failed" then
		ask(id)
		return
	end
	entry.at = lists_at()
	if lit then
		P.again = id
	end
	panel.reask(id, function()
		if P.again == id then
			P.again = nil
		end
	end)
end

-- Whether the issue open is asked for again as the lists came from Linear (the line over the page
-- stays).
function panel.again()
	return P.again ~= nil and P.open ~= nil and P.open.id == P.again
end

-- Asks the program for the issue `id`, just opened, unless what it told is of the lists that came last:
-- told under older lists, it is shown as it was told until it comes again.
local function bring(id)
	local entry = P.kept[id]
	if entry == nil or entry.status == "failed" then
		ask(id)
	elseif entry.status == "ready" and entry.at ~= lists_at() then
		entry.at = lists_at()
		panel.reask(id, function() end)
	end
end

-- Opens the issue of the list's line `item` beside the list (and asks the program for it, unless it
-- has it).
function panel.show(item)
	if item == nil or item.kind ~= "issue" then
		return false
	end
	local row = item.row
	P.pin = nil
	P.near = nil
	if P.open ~= nil and P.open.id ~= row.id then
		-- Another issue is read from its top, not from where the one before was left.
		kit.scroll_to(SCROLL, 0)
	end
	P.open = {
		id = row.id,
		identifier = row.identifier,
		title = row.title,
		priority = row.priority,
		labels = row.labels,
		state = { name = item.group.name, kind = item.group.kind },
		late = false,
	}
	bring(row.id)
	return true
end

function panel.close()
	P.open = nil
	P.pin = nil
	P.near = nil
end

-- --- Staying on an issue the person changed ----------------------------------------------------------

-- A change of the issues `ids` is on its way: where the issue open is one of them, it stays open
-- (the change may take it out of the list, and the cursor goes to its neighbour) until the cursor is
-- moved by the person.
function panel.hold(ids)
	if P.open == nil then
		return
	end
	for _, id in ids do
		if id == P.open.id then
			P.pin = { id = id, moves = list.moves() }
		end
	end
end

-- The id of the issue open where the person came to it from another and the list does not hold it, and
-- the cursor has not been moved since: the keys are its then, as its buttons are, and not those of the
-- line the cursor was left on. Else nil.
function panel.foreign()
	if P.open ~= nil and P.near == P.open.id and panel.held() then
		return P.open.id
	end
	return nil
end

-- Whether the issue open is held: it is what the person changed and the cursor has not been moved since.
function panel.held()
	local pin = P.pin
	if pin == nil then
		return false
	end
	if P.open == nil or P.open.id ~= pin.id or list.moves() ~= pin.moves then
		P.pin = nil
		return false
	end
	return true
end

-- Forgets what the program told of the issue: it has been changed, and is asked again when it is opened
-- (an answer on its way is of before).
function panel.forget(id)
	if P.open == nil or P.open.id ~= id then
		P.kept[id] = nil
		P.told[id] = question(id)
	end
end

-- The title of the issue open is made a field. The reply that gives it the keyboard.
function panel.rename()
	if P.open == nil then
		return true
	end
	return change.rename(P.open.id, panel.info().title)
end

-- The title of the issue `id` is made a field; its issue is opened beside the list first where it is
-- not, and the cursor goes to it. The reply that gives the field the keyboard.
function panel.rename_issue(id)
	-- The issue open follows the cursor: the cursor goes to the issue first, where it is another (one
	-- selected is the target, not the one under the cursor). An issue the list does not hold, that was
	-- come to from another, is read where it is.
	local current = list.current()
	if panel.foreign() ~= id and (current == nil or current.kind ~= "issue" or current.row.id ~= id) then
		if list.reveal_issue(id) == nil then
			return true
		end
	end
	if P.open == nil or P.open.id ~= id then
		panel.show(list.item(id))
	end
	return panel.rename()
end

-- The title of the issue the command `r` is for (the one issue the keys of one issue are for) is made
-- a field.
function panel.rename_target()
	local id = change.single()
	if id == nil then
		return true
	end
	return panel.rename_issue(id)
end

-- Whether the title of the issue open is being written, or a comment, or an edit of its words.
function panel.editing()
	return change.editing() or comment.typing() or edit.typing()
end

-- The keyboard goes to the field of the comment, at the end of the issue open, which is brought into
-- view. The reply.
function panel.write_comment()
	local detail = panel.detail()
	if detail == nil then
		return true
	end
	panel.scroll_by(nil, "end")
	return comment.focus(detail.id)
end

-- Begins the edit of the words of the issue open: its description (`kind` "description"), or the last
-- comment the person wrote on it ("comment"). The place is brought into view. The reply.
function panel.edit_words(kind)
	local detail = panel.detail()
	if detail == nil then
		return true
	end
	if kind == "description" then
		kit.scroll_to(SCROLL, 0, true)
		return edit.begin(detail.id, "description")
	end
	local last = edit.last_mine(detail)
	if last == nil then
		return true
	end
	return edit.begin(detail.id, last.id)
end

-- What the change part and the comment part ask of the issue open (they do not require this part).
act.panel_id = panel.id
-- How far the issue is read to (nil asks), or reads it to there: a picture put away leaves the issue where it was.
act.panel_scroll = function(y)
	if y == nil then
		return kit.offset(SCROLL)
	end
	kit.scroll_to(SCROLL, y)
end
-- The room the issue is read in, as laid out (nil where it is not).
act.panel_view = function()
	return b.layout.bounds(SCROLL)
end
act.panel_end = function()
	-- (Past the end is the end, as the next frame is laid out.)
	kit.scroll_to(SCROLL, 1e9, true)
end
act.panel_foreign = panel.foreign
act.panel_hold = panel.hold
act.panel_reask = panel.reask
act.panel_forget = panel.forget
-- What the program told of the issue `id` after it was changed, kept as it is told: no frame shows the
-- words of before.
act.panel_put = function(id, issue)
	P.told[id] = question(id)
	keep(id, issue, lists_at())
end
act.panel_listed = panel.listed
act.panel_team = function()
	return P.open ~= nil and panel.info().teamId or nil
end
-- The line of the issue open where it is no more in the list, as the lists say a line.
act.panel_row = function(id)
	if P.open ~= nil and P.open.id == id then
		return panel.detail()
	end
	return nil
end

-- Scrolls what is said of the issue by most of what its room shows, down (1) or up (-1): the way
-- the keyboard reads an issue longer than the room.
function panel.scroll(way)
	if P.open == nil then
		return false
	end
	local extent = b.layout.scroll_extent(SCROLL)
	local room = extent ~= nil and extent.view or 0
	kit.scroll_to(SCROLL, (kit.offset(SCROLL) or 0) + way * math.max(room - 48, 48), true)
	return true
end

-- Scrolls it by `points` (the arrows, where the issue has the pane), or to its top (`to` "top") or
-- its end ("end").
function panel.scroll_by(points, to)
	if P.open == nil then
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
	kit.scroll_to(SCROLL, math.min(math.max(y, 0), math.max(last, 0)), true)
	return true
end

-- Asks the program for the open issue again (where it could not tell).
function panel.retry()
	if P.open ~= nil then
		ask(P.open.id)
	end
end

-- Opens the issue in Linear, in the person's browser. The reply that does it, or nil.
function panel.link()
	local detail = panel.detail()
	if detail == nil or detail.url == nil then
		return nil
	end
	return { link = detail.url }
end

-- The issue open as it is now: from the list's line where the list has it, else from what the program
-- told of it (the issue left the list: done, given to someone else), else as it was when opened.
-- Its values and the ids of what they are (`teamId`, `stateId`, `assigneeId`, `projectId`, `cycleId`,
-- and the `estimate` and the `dueDate`).
function panel.info()
	local id = P.open.id
	local item = list.item(id)
	if item ~= nil then
		local row = item.row
		return {
			id = id,
			identifier = row.identifier,
			title = row.title,
			priority = row.priority,
			labels = row.labels,
			state = { id = row.stateId, name = item.group.name, kind = item.group.kind },
			teamId = row.teamId,
			assigneeId = row.assigneeId,
			projectId = row.projectId,
			cycleId = row.cycleId,
			estimate = row.estimate,
			dueDate = row.dueDate,
		}
	end
	local detail = panel.detail()
	if detail ~= nil then
		return {
			id = id,
			identifier = detail.identifier,
			title = detail.title,
			priority = detail.priority,
			labels = detail.labels,
			state = { id = detail.stateId, name = detail.state.name, kind = detail.state.kind },
			teamId = detail.teamId,
			assigneeId = detail.assigneeId,
			projectId = detail.projectId,
			cycleId = detail.cycleId,
			estimate = detail.estimate,
			dueDate = detail.dueDate,
		}
	end
	return {
		id = id,
		identifier = P.open.identifier,
		title = P.open.title,
		priority = P.open.priority,
		labels = P.open.labels,
		state = P.open.state,
	}
end

-- What the chips say of the issue: the names of who and what it has, from the team's choices where
-- they are here, else from what the program told of the issue.
local function shown_of(info, detail, data)
	local assignee = nil
	if info.assigneeId ~= nil then
		local member = data ~= nil and values.find(data.members, info.assigneeId) or nil
		if member ~= nil then
			assignee = { id = member.id, name = member.me and "Me" or member.name }
		elseif detail ~= nil and detail.assigneeId == info.assigneeId and detail.assignee ~= nil then
			assignee = { id = info.assigneeId, name = detail.assignee == "you" and "Me" or detail.assignee }
		else
			assignee = { id = info.assigneeId, name = "Someone" }
		end
	end
	local project = nil
	if info.projectId ~= nil then
		local known = data ~= nil and values.find(data.projects, info.projectId) or nil
		if known ~= nil then
			project = known
		elseif detail ~= nil and detail.projectId == info.projectId and detail.project ~= nil then
			project = { id = info.projectId, name = detail.project }
		else
			project = { id = info.projectId, name = "Project" }
		end
	end
	local cycle = nil
	if info.cycleId ~= nil then
		local known = data ~= nil and values.find(data.cycles, info.cycleId) or nil
		if known ~= nil then
			cycle = { id = known.id, label = known.label }
		elseif detail ~= nil and detail.cycleId == info.cycleId and detail.cycle ~= nil then
			cycle = { id = info.cycleId, label = detail.cycle }
		else
			cycle = { id = info.cycleId, label = "Cycle" }
		end
	end
	-- The points are told in the words of the team's scale (a t-shirt size), else as the number.
	local estimate = nil
	if info.estimate ~= nil then
		estimate = { label = tostring(info.estimate) }
		for _, known in data ~= nil and data.estimates or {} do
			if known.value == info.estimate then
				estimate.label = known.label
			end
		end
	end
	return {
		state = info.state,
		priority = info.priority,
		assignee = assignee,
		labels = picker.steady("panel-value:labels", info.labels),
		project = project,
		cycle = cycle,
		estimate = estimate,
		due = info.dueDate,
	}
end

-- Whether nothing of the issue is on its way and its title is not being written: the priority needs no
-- choices of the team, and is pressable then (as its key is).
local function idle(info)
	return not change.busy(info.id) and not change.editing()
end

-- Whether the other controls can be pressed too: the choices of the team are here.
local function pressable(info)
	return change.ready(info.teamId) and idle(info)
end

local function controls(info, detail)
	local data = change.options(info.teamId)
	return values.chips(shown_of(info, detail, data), {
		prefix = "panel-value:",
		disabled = not pressable(info),
		priority_disabled = not idle(info),
		busy = change.busy(info.id),
		labels = data == nil or #data.labels > 0 or #info.labels > 0,
		projects = data == nil or #data.projects > 0 or info.projectId ~= nil,
		-- (A team with no cycles, or that does not estimate, has no such chip, whatever the issue
		-- carries: the choices tell.)
		cycles = data == nil or data.cyclesEnabled == true,
		estimates = data == nil or #data.estimates > 0,
		due = true,
	})
end

-- Markdown written by someone else, drawn as text. A part that is not valid is shown as it came.
local function words(text, id)
	local tree = pictures.tree(text)
	if tree == nil then
		return b.text(text):wrap()
	end
	-- The pictures in it are drawn where they are (`pictures.lua`), as wide as the room and no wider.
	return markdown.render(tree, { id = id, gap = 8, image = pictures.drawer(P.open.id, P.room) })
end

-- The comments of the issue, the oldest first: who and when (and whether it was edited), then the words.
-- A comment of the person's has Edit comment at the end of its line, and while it is edited its words are
-- a field with Save and Cancel under it.
local function comments(detail)
	local box = b.div():id("panel-comments"):col():gap(0):mt(20)
	box:child(
		b.div():row():items_center():gap(8):mb(4):child(
			b.text("COMMENTS"):text_size(SIZE.tiny):color("dim")
		):child(b.text(tostring(#detail.comments)):text_size(SIZE.tiny):color("dim"))
	)
	local last = edit.last_mine(detail)
	for index, comment in detail.comments do
		local line = b.div():row():items_center():gap(8):mt(10):h(SIZE.small_control):no_shrink():child(
			b.text(comment.author):text_size(SIZE.small):weight(700):no_shrink()
		):child(b.text(comment.at):text_size(SIZE.tiny):color("dim"):no_shrink())
		if comment.edited then
			line:child(b.text("edited"):text_size(SIZE.tiny):color("dim"):no_shrink())
		end
		local mine = comment.mine and comment.id ~= nil and comment.id ~= ""
		if mine then
			line:child(b.spacer()):child(edit.comment_button(detail.id, comment, comment == last))
		end
		local field = mine and edit.field(detail.id, comment.id, P.room) or nil
		if field ~= nil then
			line:id("edit-top")
		end
		box:child(line)
		if field ~= nil then
			box:child(b.div():col():gap(8):mt(4):no_shrink():child(field):child(edit.actions(detail.id)))
		else
			box:child(b.div():mt(2):text_size(13.5):child(words(comment.body, "comment/" .. index)))
			local failed = mine and edit.failure(detail.id, comment.id) or nil
			if failed ~= nil then
				box:child(failed)
			end
		end
	end
	if #detail.comments == 0 then
		box:child(b.text("No comments"):text_size(SIZE.small):color("dim"))
	end
	return box
end

-- The issues round the open one, each with how it is tied: the parent, the parts, then what blocks it,
-- what it blocks and so on. A way is said once, on the first line of its run (`label`); `way` is the
-- way of the line, said by the menu of the neighbours where each line is alone.
local function near_of(detail)
	local out = {}
	if detail.parent ~= nil then
		out[#out + 1] = { label = "Parent", way = "Parent", issue = detail.parent }
	end
	for index, child in detail.children or {} do
		out[#out + 1] = { label = index == 1 and "Sub-issues" or "", way = "Sub-issue", issue = child, part = true }
	end
	local last = nil
	for _, tie in detail.ties or {} do
		out[#out + 1] = { label = tie.label ~= last and tie.label or "", way = tie.label, issue = tie.issue }
		last = tie.label
	end
	return out
end

-- One of them: its way, its identifier and its title, which a press opens. A part of the issue has the
-- picture of its state before its identifier; where there are parts, every line has the place for it, so
-- that the identifiers stand under one another.
local function near_row(entry, slot)
	local t = u.theme
	local issue = entry.issue
	local name = "near:" .. issue.id
	kit.buttons[name] = true
	local over = u:hovered(name)
	local row = b.div()
		:row()
		:items_center()
		:gap(10)
		:h(24)
		:no_shrink()
		:rounded(t.radius.control)
		:bg(over and t.control_hover or t.clear)
		:border(1, u:focus_visible(name) and t.ring or t.clear)
		:press(name)
		:focusable()
		:role("button")
		:label(entry.label .. " " .. issue.identifier .. ", " .. issue.title)
		:child(b.div():w(84):no_shrink():child(b.text(entry.label):text_size(SIZE.small):color("dim"):truncate()))
	if slot then
		row:child(b.div():w(14):no_shrink():child(entry.part and icons.state(issue.state.kind) or b.div()))
	end
	return row
		:child(b.text(issue.identifier):text_size(SIZE.small):color("dim"):no_shrink())
		:child(b.text(issue.title):text_size(13):grow(1):min_w(0):truncate())
end

-- The issue `id` that the open one is tied to, or nil.
local function near_issue(detail, id)
	for _, entry in near_of(detail) do
		if entry.issue.id == id then
			return entry.issue
		end
	end
	return nil
end

-- Opens the issue `id` that the open one is tied to. Where the list holds it the cursor goes to it and
-- the issue open follows the cursor; else it is read as it is beside the list, and stays until the
-- cursor is moved by the person. The reply.
function panel.open_near(id)
	local detail = panel.detail()
	local found = detail ~= nil and near_issue(detail, id) or nil
	if found == nil then
		return true
	end
	if list.reveal_issue(id) ~= nil then
		return true
	end
	kit.scroll_to(SCROLL, 0)
	P.open = {
		id = id,
		identifier = found.identifier,
		title = found.title,
		priority = "none",
		labels = {},
		state = { name = found.state.name, kind = found.state.kind },
		late = false,
	}
	bring(id)
	P.pin = { id = id, moves = list.moves() }
	P.near = id
	return true
end

-- The issues the open one has as parts and as ties, in the order it tells them: its neighbours but the
-- parent, which has its own key.
local function others(detail)
	local out = {}
	for _, entry in near_of(detail) do
		if entry.way ~= "Parent" then
			out[#out + 1] = entry
		end
	end
	return out
end

-- Whether the issue open has a parent, and how many other neighbours it has.
function panel.has_parent()
	local detail = panel.detail()
	return detail ~= nil and detail.parent ~= nil
end

function panel.neighbours()
	local detail = panel.detail()
	return detail ~= nil and #others(detail) or 0
end

-- Opens the parent of the issue open, as a press on its line does. The reply.
function panel.open_parent()
	local detail = panel.detail()
	if detail == nil or detail.parent == nil then
		return true
	end
	return panel.open_near(detail.parent.id)
end

local NEIGHBOURS = "neighbours-menu"
local TITLE_MOST = 45

-- A title as a line of a menu has room for it: cut, with the mark, where it is long.
local function short(title)
	local length = utf8.len(title)
	if length == nil or length <= TITLE_MOST then
		return title
	end
	local cut = string.sub(title, 1, utf8.offset(title, TITLE_MOST + 1) - 1)
	return string.gsub(cut, "%s+$", "") .. "\u{2026}"
end

-- Opens the neighbours of the issue open, but the parent: the one there is where there is one, else
-- the host's menu of them, each with its way (`ACM-389  Cache the pricing table  · Blocked by`). The
-- reply.
function panel.open_neighbours()
	local detail = panel.detail()
	local entries = detail ~= nil and others(detail) or {}
	if #entries == 0 then
		return true
	elseif #entries == 1 then
		return panel.open_near(entries[1].issue.id)
	end
	P.serial += 1
	local items = {}
	for _, entry in entries do
		local issue = entry.issue
		items[#items + 1] = {
			key = P.serial .. "|" .. issue.id,
			title = issue.identifier .. "  " .. short(issue.title) .. "  \u{b7} " .. entry.way,
		}
	end
	local where = { at = { x = S.width / 2 - 100, y = S.height / 3 } }
	if b.layout.bounds("panel-value:state") ~= nil then
		where = { near = "panel-value:state" }
	end
	return u:menu(NEIGHBOURS, items, where) or true
end

-- What a pick of that menu chose. The reply, or nil where it was not the menu's.
function panel.action(action)
	if action.from ~= NEIGHBOURS or action.kind ~= "menu" then
		return nil
	end
	local serial, id = string.match(action.entry, "^(%d+)|(.+)$")
	if serial == nil or tonumber(serial) ~= P.serial then
		return true
	end
	return panel.open_near(id)
end

-- What stands under the facts: the description and the comments, or why they are not there.
local function body(entry)
	if entry == nil or entry.status == "loading" then
		return b.div():row():items_center():gap(8):mt(16):child(kit.spinner(13, "dim")):child(
			b.text("Loading"):text_size(SIZE.small):color("dim")
		)
	elseif entry.status == "failed" then
		return b.div():col():gap(10):mt(16):items_start():child(
			b.text(entry.error):color("dim"):wrap():text_size(SIZE.small)
		):child(kit.button("issue-retry", "Try again"))
	end
	local detail = entry.issue
	local box = b.div():col():text_size(13.5)
	local near = near_of(detail)
	if #near > 0 then
		local rows = b.div():col():gap(2):mb(16)
		local slot = detail.children ~= nil and #detail.children > 0
		for _, one in near do
			rows:child(near_row(one, slot))
		end
		box:child(rows)
	end
	-- The description, or, while it is edited, the field that holds it whole; under it Attach file and Edit
	-- description.
	local about = edit.field(detail.id, "description", P.room)
	local editing = about ~= nil
	if about == nil and detail.description ~= nil and detail.description ~= "" then
		about = words(detail.description, "description")
	elseif about == nil then
		about = b.text("No description"):text_size(SIZE.small):color("dim")
	end
	box:child(attach.zone(detail.id, about, {
		edit = edit.description_button(detail.id),
		actions = editing and edit.actions(detail.id) or nil,
		failure = not editing and edit.failure(detail.id, "description") or nil,
	}))
	box:child(comments(detail))
	box:child(comment.view(detail.id))
	if detail.more then
		box:child(
			b.text("There is more in Linear."):text_size(SIZE.small):color("dim"):mt(12):mb(4)
		)
	end
	return box
end

-- The panel, `width` points wide: its head (the buttons, the identifier and Share), the title and the
-- facts, then what scrolls.
function panel.view(width)
	local info = P.open
	P.room = math.max(width - 2 * SIZE.pad - 2, 120)
	local entry = P.kept[info.id]
	local detail = entry ~= nil and entry.status == "ready" and entry.issue or nil
	local top = b.div():col():no_shrink():pt(SIZE.head_top):px(SIZE.pad):pb(14)
	-- What is done with the issue, over what it is: to work on it at the start of the line (Work on is
	-- the issue's own, here and nowhere else), to leave it at the end.
	local buttons = b.div():row():items_center():gap(8):no_shrink()
	buttons:child(kit.button("work", "Work on issue", {
		primary = true,
		suffix = commands.label("work"),
		tip = "Start an agent on it, in a pane beside this one",
	}))
	-- Esc is written on the one place it does something: Close, unless the keyboard is in the comment or in
	-- an edit (the line of keys says Esc leaves) or files wait for the description or an edit is open
	-- (Cancel says it). Close is as wide as it is
	-- with the key, so that nothing beside it moves when the key goes.
	local at = b.layout.bounds("panel-close")
	if at ~= nil and P.close_keyed then
		P.close_width = at.w
	end
	P.close_keyed = not comment.typing() and not edit.typing() and not attach.cancelable() and not edit.cancelable()
	buttons:child(b.spacer()):child(kit.button("open-linear", "Open in Linear", {
		suffix = commands.label("link"),
		disabled = detail == nil,
		tip = detail == nil and "Linear has not told where it is" or "Open the issue in your browser",
	})):child(kit.button("panel-close", "Close", { suffix = P.close_keyed and "Esc" or nil }):min_w(P.close_width or 0))
	top:child(buttons)
	-- What the issue is, and at the end of its line Share: the identifier, the title and the link on the
	-- pasteboard, one line to paste where the issue is shared. Dim where Linear has not told the link.
	local shared = change.row(info.id)
	local linked = shared ~= nil and shared.url ~= nil and shared.url ~= ""
	top:child(
		b.div():row():items_center():gap(8):mt(9):no_shrink()
			:child(b.text(info.identifier):text_size(SIZE.small):color("dim"):truncate():min_w(0))
			:child(b.spacer())
			:child(kit.button("panel-share", "Share", {
				small = true,
				suffix = commands.label("share"),
				disabled = not linked,
				tip = linked and "Copy its identifier, title and link, to paste where you share it"
					or "Linear has not told where it is",
			}))
	)
	local now = panel.info()
	-- The title is the words, which a press turns into a field; a press while it is a field is its own.
	local title = b.text(now.title):text_size(16):weight(700):wrap():no_shrink()
	kit.buttons["panel-title-edit"] = true
	local edit = b.div()
		:press("panel-title-edit")
		:focusable()
		:role("button")
		:label("Edit the title")
		:tip("Edit the title")
		:mt(3)
		:mb(10)
		:no_shrink()
		:child(title)
	top:child(change.title(now.id, edit))
	top:child(controls(now, detail))
	local why = change.error_for(now.id)
	if why ~= nil then
		top:child(
			b.div():row():items_start():gap(8):no_shrink():mt(10):child(
				b.div():size(7, 7):rounded(4):bg("warning"):no_shrink():mt(6)
			):child(b.text(why):text_size(SIZE.small):color("warning"):wrap():min_w(0):grow(1))
		)
	end
	local content = b.div():col():grow(1):min_w(0):min_h(0):child(top)
	content:child(b.div():h(1):bg(b.alpha("foreground", 0.18)):no_shrink())
	content:child(
		b.div():id(SCROLL):grow(1):min_h(0):scroll():col():pt(14):px(SIZE.pad):pb(SIZE.pad):child(body(entry))
	)
	local side = b.div():row():w(width):no_shrink():min_h(0)
	if width < S.width then
		side:child(b.div():w(1):bg(b.alpha("foreground", 0.23)):no_shrink())
	end
	return side:child(content)
end

return panel
