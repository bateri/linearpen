-- What a right press on a line opens, and what is copied: the host's menu with the values to change
-- and the rest of what is done to an issue (the keys of the card, said once more where the pointer is),
-- and the id, the branch name and the link of an issue put on the pasteboard, which the kit's reply
-- `copy` does for a press, a pick of a menu or a command. The menu is of the issues selected where the
-- line pressed is one of them, else of that line alone (what is selected stays).

local kit = require("../lib/kit")
local change = require("./change")
local list = require("./list")
local panel = require("./panel")

local b, u, S = kit.b, kit.u, kit.S

local context = {}

local MENU = "context-menu"

-- The menu that is open: its serial (a pick of an older menu is not one), the issues, and where it was
-- opened (what a value's menu opens at).
local X = { serial = 0, ids = {}, at = nil }

-- --- Copies ------------------------------------------------------------------------------------------

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
}

-- A control character in what Linear sends is never put on the pasteboard (a terminal would take it
-- for its own).
local function plain(text)
	return (string.gsub(text, "%c", ""))
end

-- Copies the id, the branch name or the link (`kind`) of the issue `id`, and says so. The reply.
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

local function entry(key, title, keys, enabled)
	return { key = X.serial .. "|" .. key, title = title, keys = keys, enabled = enabled }
end

-- The entries of the menu of `ids`: the values to change, and, of one issue, the rest.
local function entries(ids)
	local list_ = {
		entry("state", "Status", "s"),
		entry("priority", "Priority", "p"),
		entry("assignee", "Assignee", "a"),
		entry("assign_me", "Assign to me", "i"),
		entry("labels", "Labels", "l"),
		entry("project", "Project", "shift+p"),
	}
	-- A team that has no cycles, or does not estimate, has none of them to change.
	if change.offers("cycle", ids) then
		list_[#list_ + 1] = entry("cycle", "Cycle", "shift+c")
	end
	if change.offers("estimate", ids) then
		list_[#list_ + 1] = entry("estimate", "Estimate", "shift+e")
	end
	list_[#list_ + 1] = entry("due", "Due date", "shift+d")
	if #ids ~= 1 then
		return list_
	end
	local row = change.row(ids[1])
	list_[#list_ + 1] = entry("title", "Rename", "r")
	list_[#list_ + 1] = "-"
	list_[#list_ + 1] = entry("copy_id", "Copy ID", "mod+shift+i")
	list_[#list_ + 1] = entry("copy_branch", "Copy branch name", "mod+shift+.", row ~= nil and row.branchName ~= nil)
	list_[#list_ + 1] = entry("copy_link", "Copy link", "mod+shift+,")
	list_[#list_ + 1] = "-"
	list_[#list_ + 1] = entry("link", "Open in Linear", "shift+o")
	return list_
end

-- A right press: the menu of the line it landed on, at the place of the press. The reply, or nil where
-- it was not on a line of an issue.
function context.press(event)
	if (event.button or "left") ~= "right" then
		return nil
	end
	local hit = b.layout.hit({ x = event.x, y = event.y })
	local key = hit ~= nil and (string.match(hit, "^row:(.+)$") or string.match(hit, "^check:(.+)$")) or nil
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
	elseif key == "link" and #ids == 1 then
		return context.link(id) or true
	end
	return true
end

return context
