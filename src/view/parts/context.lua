-- What a right press on a line opens, and what is copied: the host's menu with the values to change
-- and the rest of what is done to an issue (the keys of the card, said once more where the pointer is),
-- and the id, the branch name, the link of an issue, or the line to share it by, put on the pasteboard,
-- which the kit's reply `copy` does for a press, a pick of a menu or a command. The menu is of the issues selected where the
-- line pressed is one of them, else of that line alone (what is selected stays).

local kit = require("../lib/kit")
local change = require("./change")
local commands = require("./commands")
local list = require("./list")
local panel = require("./panel")
local row_ = require("./row")

local b, u, S, act = kit.b, kit.u, kit.S, kit.act

local context = {}

local MENU = "context-menu"

-- The menu that is open: its serial (a pick of an older menu is not one), the issues, and where it was
-- opened (what a value's menu opens at).
local X = { serial = 0, ids = {}, at = nil }

-- --- Copies ------------------------------------------------------------------------------------------

-- The controls of the direction of text (U+202A to U+202E, U+2066 to U+2069): a title Linear sends may
-- hold them, and they would turn the words around wherever the line is pasted.
local function undirected(text)
	text = string.gsub(text, "\xE2\x80[\xAA-\xAE]", "")
	return (string.gsub(text, "\xE2\x81[\xA6-\xA9]", ""))
end

-- What of an issue is copied, and what is said of it.
local COPIES = {
	id = {
		of = function(row)
			return row.identifier, "Copied " .. row.identifier
		end,
	},
	branch = {
		of = function(row)
			return row.branchName, "Copied the branch name"
		end,
		missing = "Linear has no branch name for this issue",
	},
	link = {
		of = function(row)
			return row.url, "Copied the link"
		end,
		missing = "Linear has not told where this issue is",
	},
	-- The line to paste where the issue is shared: its identifier, its title and its link, as a line
	-- dragged out of the list carries it. Without the link there is nothing to share.
	share = {
		of = function(row)
			if row.url == nil or row.url == "" then
				return nil
			end
			return undirected(row_.words(row)) .. " " .. row.url, "Copied " .. row.identifier .. " with its title and link"
		end,
		missing = "Linear has not told where this issue is",
	},
}

-- A control character in what Linear sends is never put on the pasteboard (a terminal would take it
-- for its own).
local function plain(text)
	return (string.gsub(text, "%c", ""))
end

-- Copies the id, the branch name, the link or the line to share (`kind`) of the issue `id`, and says so.
-- The reply.
function context.copy(kind, id)
	local row = id ~= nil and change.row(id) or nil
	local copy = COPIES[kind]
	if row == nil or copy == nil then
		return true
	end
	local text, words = copy.of(row)
	text = text ~= nil and plain(text) or ""
	if text == "" then
		kit.lib.toast(S.toasts, copy.missing, "warn")
		return true
	end
	kit.lib.toast(S.toasts, words)
	return { copy = text }
end

-- The one issue the keys of one issue are for (the issue read, else the one selected, else the one under
-- the cursor), or nil.
function context.single()
	return change.single()
end

-- Opens the issue in Linear, in the person's browser. The reply, or nil.
function context.link(id)
	local row = id ~= nil and change.row(id) or nil
	if row == nil or row.url == nil or row.url == "" then
		return nil
	end
	return { link = row.url }
end

-- --- The menu ----------------------------------------------------------------------------------------

-- (The keys of an entry are the command's, from the one table: the menu says what the card says.)
local function entry(key, title, command, enabled)
	return { key = X.serial .. "|" .. key, title = title, keys = commands.first(command), enabled = enabled }
end

-- The entries of the menu of `ids`: the values to change, and, of one issue, the rest.
local function entries(ids)
	local list_ = {
		entry("state", "Status", "status"),
		entry("priority", "Priority", "priority"),
		entry("assignee", "Assignee", "assignee"),
		entry("assign_me", "Assign to me", "assign_me"),
		entry("labels", "Labels", "labels"),
		entry("project", "Project", "project"),
	}
	-- A team that has no cycles, or does not estimate, has none of them to change.
	if change.offers("cycle", ids) then
		list_[#list_ + 1] = entry("cycle", "Cycle", "cycle")
	end
	if change.offers("estimate", ids) then
		list_[#list_ + 1] = entry("estimate", "Estimate", "estimate")
	end
	list_[#list_ + 1] = entry("due", "Due date", "due")
	if #ids ~= 1 then
		return list_
	end
	local row = change.row(ids[1])
	list_[#list_ + 1] = entry("title", "Rename", "title")
	list_[#list_ + 1] = "-"
	list_[#list_ + 1] = entry("copy_id", "Copy ID", "copy_id")
	list_[#list_ + 1] = entry("copy_branch", "Copy branch name", "copy_branch", row ~= nil and row.branchName ~= nil)
	list_[#list_ + 1] = entry("copy_link", "Copy link", "copy_link")
	list_[#list_ + 1] = entry("share", "Share", "share")
	list_[#list_ + 1] = "-"
	list_[#list_ + 1] = entry("link", "Open in Linear", "link")
	list_[#list_ + 1] = entry("go_to_agent", "Go to the agent", "go_to_agent", act.agent_of(ids[1]) ~= nil)
	-- (A line of the inbox: its notifications, read in Linear.)
	if S.scope == "inbox" then
		list_[#list_ + 1] = entry("mark_read", "Mark read", "mark_read", act.unread(ids[1]))
	end
	return list_
end

-- A right press: the menu of the line it landed on, at the place of the press. The reply, or nil where
-- it was not on a line of an issue.
function context.press(event)
	if (event.button or "left") ~= "right" then
		return nil
	end
	local hit = b.layout.hit({ x = event.x, y = event.y })
	local agent = hit ~= nil and string.match(hit, "^agent:(.+)$") or nil
	local key = hit ~= nil and (string.match(hit, "^row:(.+)$") or string.match(hit, "^check:(.+)$")) or nil
	if agent ~= nil then
		-- (The agent's mark is the line's too.)
		key = "i:" .. agent
	end
	local item = key ~= nil and list.item_of(key) or nil
	if item == nil or item.kind ~= "issue" then
		return nil
	end
	local ids = list.is_marked(key) and list.marked_ids() or { item.row.id }
	-- The cursor goes to the line pressed: the issue open follows it, and a key that follows is its.
	list.focus(key)
	X.serial += 1
	X.ids = ids
	X.at = { x = event.x, y = event.y }
	return u:menu(MENU, entries(ids), { at = X.at }) or true
end

-- What a pick of the menu chose. The reply, or nil where it was not the menu's.
function context.action(action)
	if action.from ~= MENU or action.kind ~= "menu" then
		return nil
	end
	local serial, key = string.match(action.entry, "^(%d+)|(.+)$")
	if serial == nil or tonumber(serial) ~= X.serial then
		return true
	end
	local ids, id = X.ids, X.ids[1]
	if
		key == "state"
		or key == "priority"
		or key == "assignee"
		or key == "labels"
		or key == "project"
		or key == "cycle"
		or key == "estimate"
		or key == "due"
	then
		return change.open(key, ids, nil, X.at)
	elseif key == "assign_me" then
		return change.assign_me(ids)
	elseif key == "title" and #ids == 1 then
		return panel.rename_issue(id)
	elseif key == "copy_id" and #ids == 1 then
		return context.copy("id", id)
	elseif key == "copy_branch" and #ids == 1 then
		return context.copy("branch", id)
	elseif key == "copy_link" and #ids == 1 then
		return context.copy("link", id)
	elseif key == "share" and #ids == 1 then
		return context.copy("share", id)
	elseif key == "link" and #ids == 1 then
		return context.link(id) or true
	elseif key == "go_to_agent" and #ids == 1 then
		return act.go_to_agent(id)
	elseif key == "mark_read" and #ids == 1 then
		return act.mark_read(id)
	end
	return true
end

return context
