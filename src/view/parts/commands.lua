-- The commands of the page, once: the id, the title, the group and the keys each has. The palette,
-- the card of keys, the status line and the host's list of commands are all made of this table, so
-- that a key is written in one place. `native` are the keys the page reads itself from the
-- keyboard (the cursor's); they are listed and shown, but not given to the host, which would take
-- them from the field that has the keyboard.

local kit = require("../lib/kit")

local commands = {}

local TABLE = {
	{ id = "down", title = "Next issue", group = "Move", keys = { "down", "j" }, native = true },
	{ id = "up", title = "Previous issue", group = "Move", keys = { "up", "k" }, native = true },
	{ id = "first", title = "First issue", group = "Move", keys = { "home" }, native = true },
	{ id = "last", title = "Last issue", group = "Move", keys = { "end" }, native = true },
	{ id = "fold", title = "Close the group", group = "Move", keys = { "left" }, when = "list" },
	{ id = "unfold", title = "Open the group", group = "Move", keys = { "right" }, when = "list" },
	{ id = "select", title = "Select the issue", group = "Select", keys = { "x" }, when = "list" },
	{ id = "select_more", title = "Select more", group = "Select", keys = { "shift+up", "shift+down" }, native = true },
	{ id = "select_all", title = "Select all", group = "Select", keys = { "mod+a" }, when = "idle" },
	{ id = "status", title = "Status", group = "Change", keys = { "s" }, when = "changeable" },
	{ id = "priority", title = "Priority", group = "Change", keys = { "p" }, when = "plain_change" },
	{ id = "assignee", title = "Assignee", group = "Change", keys = { "a" }, when = "changeable" },
	{ id = "labels", title = "Labels", group = "Change", keys = { "l" }, when = "changeable" },
	{ id = "project", title = "Project", group = "Change", keys = { "shift+p" }, when = "changeable" },
	{ id = "title", title = "Title", group = "Change", keys = { "e" }, when = "titleable" },
	{ id = "open", title = "Open beside the list", group = "Issue", keys = { "enter" }, native = true },
	{ id = "link", title = "Open in Linear", group = "Issue", keys = { "o" }, when = "issue" },
	{ id = "read_down", title = "Scroll the issue down", group = "Issue", keys = { "space" }, when = "reading" },
	{ id = "read_up", title = "Scroll the issue up", group = "Issue", keys = { "shift+space" }, when = "reading" },
	{ id = "work", title = "Work on the issue, or the ones selected", group = "Issue", keys = { "w" }, when = "workable" },
	{ id = "new", title = "New issue", group = "Issue", keys = { "c" }, when = "idle" },
	{ id = "next_field", title = "Next field", group = "Work on, New issue", keys = { "tab" }, native = true },
	{ id = "choose", title = "Choose", group = "Work on, New issue", keys = { "left", "right", "enter" }, native = true },
	{ id = "start", title = "Start the agent", group = "Work on, New issue", keys = { "mod+enter" }, when = "startable" },
	{ id = "create", title = "Create the issue", group = "Work on, New issue", keys = { "mod+enter" }, when = "creatable" },
	{ id = "scope_mine", title = "Show my issues", group = "Everywhere", keys = { "1" }, when = "list" },
	{ id = "scope_all", title = "Show all issues", group = "Everywhere", keys = { "2" }, when = "list" },
	{ id = "filter", title = "Filter issues", group = "Everywhere", keys = { "/" }, when = "list" },
	{ id = "refresh", title = "Ask Linear again", group = "Everywhere", keys = { "r" }, when = "keyed" },
	{ id = "palette", title = "Find a command", group = "Everywhere", keys = { "mod+k" }, when = "chord" },
	{ id = "help", title = "Show the keys", group = "Everywhere", keys = { "?" }, when = "list" },
	{
		id = "back",
		title = "Go back",
		group = "Everywhere",
		keys = { "escape" },
		native = true,
	},
}

-- The conditions, by name; page.lua gives them: what the page shows, never where the keyboard is
-- (a condition that must know the keyboard says so in its name: `idle`).
commands.when = {}

function commands.all()
	return TABLE
end

function commands.get(id)
	for _, command in TABLE do
		if command.id == id then
			return command
		end
	end
	return nil
end

-- The words of the first key of `id`, or nil.
function commands.first(id)
	local command = commands.get(id)
	return command ~= nil and command.keys[1] or nil
end

-- The first key of `id` as this keyboard writes it.
function commands.label(id)
	local first = commands.first(id)
	return first ~= nil and kit.keys_of(first) or nil
end

-- Whether `command` can be done now.
function commands.enabled(command)
	local name = command.when
	if name == nil then
		return true
	end
	local when = commands.when[name]
	return when ~= nil and when() == true
end

-- The entries given to the library, by id: the library keeps these tables and writes their keys
-- every time the page is drawn, so that `settle` can change them.
local LIBRARY = {}

-- The list for the library's `ui.commands`: what the host is given, each with its `when` made a
-- function.
function commands.for_library()
	local out = {}
	for _, command in TABLE do
		if not command.native then
			local entry = {
				id = command.id,
				title = command.title,
				group = command.group,
				keys = command.keys,
				when = function()
					return commands.enabled(command)
				end,
			}
			LIBRARY[command.id] = entry
			out[#out + 1] = entry
		end
	end
	return out
end

-- Whether `command` lists `words` among its keys.
local function lists(command, words)
	for _, keys in command.keys do
		if keys == words then
			return true
		end
	end
	return false
end

-- Commands share keys (Command+Enter starts the agent in one form and makes the issue in the other),
-- and the host gives a key to the first command that lists it, whether it can be done or not. So a
-- key that several list is written on one of them: the first that can be done now, else the first.
-- Before the page is drawn.
function commands.settle()
	for _, command in TABLE do
		local entry = LIBRARY[command.id]
		if entry ~= nil then
			local kept = {}
			for _, words in command.keys do
				local owner = nil
				for _, other in TABLE do
					if not other.native and lists(other, words) then
						if owner == nil or (not commands.enabled(owner) and commands.enabled(other)) then
							owner = other
						end
					end
				end
				if owner == command then
					kept[#kept + 1] = words
				end
			end
			entry.keys = kept
		end
	end
end

-- The commands that can be done now and are not the cursor's keys, as the items of the palette.
function commands.items()
	local items = {}
	for _, command in TABLE do
		if not command.native and commands.enabled(command) then
			items[#items + 1] = {
				key = command.id,
				title = command.title,
				hint = command.group,
				keys = command.keys,
			}
		end
	end
	return items
end

return commands
