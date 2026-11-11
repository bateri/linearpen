-- The issue beside the list: what a row does not carry (its description and its comments, asked of the
-- program when the issue is opened and kept), under the identifier, the title and what the row knew.
-- The text of an issue is written by others: it is shown as text (markdown, drawn by the kit's
-- library), and a link in it opens only where the person presses it.

local markdown = require("@pencere/markdown")
local kit = require("../lib/kit")
local commands = require("./commands")
local icons = require("./icons")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local panel = {}

-- The issue open: what the row knew of it (`{ id, identifier, title, priority, labels, state, late }`),
-- and what the program told of it, by id: `{ status = "loading" | "ready" | "failed", issue, error }`.
local P = { open = nil, kept = {} }
panel.state = P

local PRIORITY = { urgent = "Urgent", high = "High", medium = "Medium", low = "Low", none = "No priority" }

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

-- Opens the issue of the list's line `item` beside the list (and asks the program for it, unless it
-- has it).
function panel.show(item)
	if item == nil or item.kind ~= "issue" then
		return false
	end
	local row = item.row
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

-- A line of what is known of the issue: a small picture and its words.
local function fact(picture, words)
	return b.div():row():items_center():gap(7):no_shrink():child(picture):child(
		b.text(words):text_size(SIZE.small):color("dim"):no_shrink()
	)
end

local function meta(info, detail)
	local row = b.div():row():wrap():items_center():gap_x(16):gap_y(6)
	row:child(fact(icons.state(info.state.kind, info.late), info.state.name))
	row:child(fact(icons.priority(info.priority), PRIORITY[info.priority] or info.priority))
	for _, label in info.labels do
		row:child(fact(icons.dot(label.color), label.name))
	end
	if detail ~= nil then
		if detail.cycle ~= nil then
			row:child(b.text("Cycle " .. detail.cycle):text_size(SIZE.small):color("dim"):no_shrink())
		end
		if detail.project ~= nil then
			row:child(b.text(detail.project):text_size(SIZE.small):color("dim"):no_shrink())
		end
		if detail.assignee ~= nil and detail.assignee ~= "you" then
			row:child(b.text(detail.assignee):text_size(SIZE.small):color("dim"):no_shrink())
		end
	end
	return row
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
	local box = b.div():col():gap(0):mt(20)
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
	top:child(b.text(info.title):text_size(16):weight(700):wrap():mt(14):mb(10):no_shrink())
	top:child(meta(info, detail))
	local content = b.div():col():grow(1):min_w(0):min_h(0):child(top)
	content:child(b.div():h(1):bg(b.alpha("foreground", 0.18)):no_shrink())
	content:child(
		b.div():id("panel-scroll"):grow(1):min_h(0):scroll():col():pt(14):px(SIZE.pad):pb(SIZE.pad):child(body(entry))
	)
	local side = b.div():row():w(width):no_shrink():min_h(0)
	if width < S.width then
		side:child(b.div():w(1):bg(b.alpha("foreground", 0.23)):no_shrink())
	end
	return side:child(content)
end

return panel
