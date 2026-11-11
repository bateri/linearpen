-- The issue beside the list: what a row does not carry (its description and its comments, asked of the
-- program when the issue is opened and kept), under the identifier, the title and what the row knew.
-- The text of an issue is written by others: it is shown as text (markdown, drawn by the kit's
-- library), and a link in it opens only where the person presses it.

local markdown = require("@pencere/markdown")
local kit = require("../lib/kit")
local change = require("./change")
local commands = require("./commands")
local list = require("./list")
local values = require("./values")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local panel = {}

-- The issue open: what the row knew of it when it was opened (`{ id, identifier, title, priority, labels,
-- state }`: all it says where the list has the issue no more and the program has not told yet), and
-- what the program told of it, by id: `{ status = "loading" | "ready" | "failed", issue, error }`.
-- `pin` is set while a change of the issue is on its way or has just been made: the issue stays open
-- (it may have left the list) until the cursor is moved by the person (`{ id, moves }`).
local P = { open = nil, kept = {}, pin = nil }
panel.state = P

-- The box that scrolls: the description and the comments.
local SCROLL = "panel-scroll"

-- The room an issue takes beside the list (the page is this wide), points: a part of the page, held
-- between two widths; a pane too narrow for both takes the whole of it.
function panel.width(page)
	if page < 760 then
		return page
	end
	return math.floor(math.min(520, math.max(380, page * 0.44)) + 0.5)
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

local function ask(id)
	P.kept[id] = { status = "loading" }
	kit.call("issue", { id = id }, function(result, err)
		local entry = P.kept[id]
		if err then
			entry.status, entry.error = "failed", err.message or err.code
		else
			entry.status, entry.issue = "ready", result
		end
		return true
	end)
end

-- Asks again for the issue open, without a loading: what is shown stays until it comes. `done` is
-- called when it has. Whether it asked (it does not where the issue is not the one open).
function panel.reask(id, done)
	if P.open == nil or P.open.id ~= id then
		return false
	end
	kit.call("issue", { id = id }, function(result, err)
		if not err then
			P.kept[id] = { status = "ready", issue = result }
		end
		done()
		return true
	end)
	return true
end

-- Opens the issue of the list's line `item` beside the list (and asks the program for it, unless it
-- has it).
function panel.show(item)
	if item == nil or item.kind ~= "issue" then
		return false
	end
	local row = item.row
	P.pin = nil
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
	local entry = P.kept[row.id]
	if entry == nil or entry.status == "failed" then
		ask(row.id)
	end
	return true
end

function panel.close()
	P.open = nil
	P.pin = nil
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

-- The change of the issue `id` is done and the list still holds it: it need not be held open.
function panel.release(id)
	if P.pin ~= nil and P.pin.id == id then
		P.pin = nil
	end
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

-- Forgets what the program told of the issue: it has been changed, and is asked again when it is opened.
function panel.forget(id)
	if P.open == nil or P.open.id ~= id then
		P.kept[id] = nil
	end
end

-- The title of the issue open is made a field. The reply that gives it the keyboard.
function panel.rename()
	if P.open == nil then
		return true
	end
	return change.rename(P.open.id, panel.info().title)
end

-- The title of the issue the command `e` is for (the one target) is made a field; its issue is opened
-- beside the list first where it is not.
function panel.rename_target()
	local ids = change.targets()
	if #ids ~= 1 then
		return true
	end
	-- The issue open follows the cursor: the cursor goes to the issue first, where it is another (one
	-- selected is the target, not the one under the cursor).
	local current = list.current()
	if current == nil or current.kind ~= "issue" or current.row.id ~= ids[1] then
		if list.reveal_issue(ids[1]) == nil then
			return true
		end
	end
	if P.open == nil or P.open.id ~= ids[1] then
		panel.show(list.item(ids[1]))
	end
	return panel.rename()
end

-- Whether the title of the issue open is being written.
function panel.editing()
	return change.editing()
end

-- What the change part asks of the issue open (it does not require this part).
act.panel_id = panel.id
act.panel_hold = panel.hold
act.panel_release = panel.release
act.panel_reask = panel.reask
act.panel_forget = panel.forget
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
-- Its values and the ids of what they are (`teamId`, `stateId`, `assigneeId`, `projectId`).
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
	return {
		state = info.state,
		priority = info.priority,
		assignee = assignee,
		labels = info.labels,
		project = project,
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

-- What stands after the controls, plain: the cycle.
local function cycle(detail)
	if detail ~= nil and detail.cycle ~= nil then
		return b.div():h(SIZE.control):row():items_center():no_shrink():child(
			b.text("Cycle " .. detail.cycle):text_size(SIZE.small):color("dim"):no_shrink()
		)
	end
	return nil
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
		tail = cycle(detail),
	})
end

-- Markdown written by someone else, drawn as text. A part that is not valid is shown as it came.
local function words(text, id)
	local ok, tree = pcall(markdown.parse, text)
	if not ok then
		return b.text(text):wrap()
	end
	return markdown.render(tree, { id = id, gap = 8 })
end

local function comments(detail)
	local box = b.div():id("panel-comments"):col():gap(0):mt(20)
	box:child(
		b.div():row():items_center():gap(8):mb(4):child(
			b.text("COMMENTS"):text_size(SIZE.tiny):color("dim")
		):child(b.text(tostring(#detail.comments)):text_size(SIZE.tiny):color("dim"))
	)
	for index, comment in detail.comments do
		box:child(
			b.div():row():items_center():gap(8):mt(10):child(b.text(comment.author):text_size(SIZE.small):weight(700)):child(
				b.text(comment.at):text_size(SIZE.tiny):color("dim")
			)
		)
		box:child(b.div():mt(2):text_size(13.5):child(words(comment.body, "comment/" .. index)))
	end
	if #detail.comments == 0 then
		box:child(b.text("No comments"):text_size(SIZE.small):color("dim"))
	end
	return box
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
	if detail.description ~= nil and detail.description ~= "" then
		box:child(words(detail.description, "description"))
	else
		box:child(b.text("No description"):text_size(SIZE.small):color("dim"))
	end
	box:child(comments(detail))
	if detail.more then
		box:child(
			b.text("There is more in Linear."):text_size(SIZE.small):color("dim"):mt(12):mb(4)
		)
	end
	return box
end

-- The panel, `width` points wide: its head (the identifier and the two buttons), the title and the
-- facts, then what scrolls.
function panel.view(width)
	local info = P.open
	local entry = P.kept[info.id]
	local detail = entry ~= nil and entry.status == "ready" and entry.issue or nil
	local top = b.div():col():no_shrink():pt(SIZE.head_top):px(SIZE.pad):pb(14)
	top:child(
		b.div():row():items_center():gap(8):no_shrink():child(
			b.text(info.identifier):text_size(SIZE.small):color("dim")
		):child(b.spacer()):child(kit.button("open-linear", "Open in Linear", {
			suffix = commands.label("link"),
			disabled = detail == nil,
			tip = detail == nil and "Linear has not told where it is" or "Open the issue in your browser",
		})):child(kit.button("panel-close", "Close", { suffix = "Esc" }))
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
		:mt(14)
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
