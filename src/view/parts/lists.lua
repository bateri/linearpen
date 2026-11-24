-- The card of lists, which the button of the head and `g` open: the lists a person can go to, one a
-- line with what it is and, where it has one, its key. It is the kit's palette (a field that finds,
-- the arrows, Enter, Esc). My issues and All issues come first (`1`, `2`), then the issues the person
-- made and those they are subscribed to (no key: 3 to 9 are the favorites'); then what Linear starred
-- (`3` to `9` for the first seven), the teams, the saved views and the projects, as the program
-- tells them (`S.catalog`). What each is is said at the end of its line, in words, because the card
-- cannot draw the heads of its sections.

local kit = require("../lib/kit")

local u, S, act = kit.u, kit.S, kit.act

local lists = {}

local L = { open = false, state = nil, serial = 0, built = nil, version = nil, items = nil, names = {} }
lists.state = L

-- The lists every workspace has: the person's issues, the workspace's, the inbox and the projects first; the
-- person's in the two other ways Linear's My issues has them after the favorites (`AFTER`).
local OWN = {
	{ key = "mine", title = "My issues", hint = "Assigned to you", keys = "1", list = { kind = "mine" } },
	{ key = "all", title = "All issues", hint = "The workspace", keys = "2", list = { kind = "all" } },
	{ key = "inbox", title = "Inbox", hint = "Your notifications", list = { kind = "inbox" } },
	{ key = "projects", title = "Projects", hint = "The workspace's projects", list = { kind = "projects" } },
	{ key = "created", title = "Created by me", hint = "Made by you", list = { kind = "created" }, after = true },
	{ key = "subscribed", title = "Subscribed", hint = "You follow them", list = { kind = "subscribed" }, after = true },
}

-- What a list of the program's says of what it is.
local KINDS = { view = "View", project = "Project", team = "Team", label = "Label", cycle = "Cycle", search = "Search" }

local function hint_of(item)
	if item.kind == "cycle" and item.cycle ~= nil then
		return string.format("Cycle %d, ends %s", item.cycle.number, item.cycle.ends)
	end
	local words = KINDS[item.kind] or item.kind
	if item.kind == "view" and item.team ~= nil then
		words = words .. " \u{b7} " .. item.team
	end
	return words
end

-- The key of the list the page shows, as the program names it: `mine`, `all`, `view:<id>`.
function lists.current()
	if S.list_id ~= nil then
		return S.scope .. ":" .. S.list_id
	end
	return S.scope
end

-- The items of the card: the own two, then the program's, with the keys of the first seven favorites.
-- Made again only when what the program tells changed (the palette keeps its search while the items
-- are the same, and a change of `S.catalog` is a new table with the same lists in it).
local function items()
	local catalog = S.catalog
	if L.built == catalog and L.items ~= nil then
		return L.items
	end
	local out, keys = {}, {}
	for _, own in OWN do
		if not own.after then
			out[#out + 1] = own
			keys[#keys + 1] = own.key
		end
	end
	local placed = false
	local function after()
		placed = true
		for _, own in OWN do
			if own.after then
				out[#out + 1] = own
				keys[#keys + 1] = own.key
			end
		end
	end
	local number = 3
	if catalog ~= nil then
		for _, item in catalog.items or {} do
			if not placed and item.section ~= "favorite" then
				after()
			end
			local entry = {
				key = item.key,
				title = item.title,
				hint = hint_of(item),
				list = { kind = item.kind, id = item.id },
			}
			if item.section == "favorite" and number <= 9 then
				entry.keys = tostring(number)
				number += 1
			end
			out[#out + 1] = entry
			-- (A view that was renamed, or a cycle whose end moved, is the same list with other words.)
			keys[#keys + 1] = item.key .. "\n" .. item.title .. "\n" .. (entry.hint or "") .. "\n" .. (entry.keys or "")
		end
		if not placed then
			after()
		end
		-- Where the names have not come, or could not be asked for, the card says so in a line that
		-- goes nowhere.
		if not catalog.loaded then
			local failed = catalog.error ~= nil
			out[#out + 1] = {
				key = "names:note",
				title = failed and "The other lists could not be asked for" or "Asking Linear for the other lists",
				hint = failed and "Refresh asks again" or nil,
				enabled = false,
			}
			keys[#keys + 1] = "names:note"
		end
	end
	if not placed then
		after()
	end
	local version = table.concat(keys, "|")
	-- (The same lists keep the same items, so that the search is not made again at every push.)
	if L.version ~= version then
		L.version, L.items = version, out
	end
	L.built = catalog
	return L.items
end

-- The list the person's `n`th favorite is (the key `n + 2` goes to it), or nil where there is none.
function lists.favorite(n)
	local catalog = S.catalog
	if catalog == nil then
		return nil
	end
	local seen = 0
	for _, item in catalog.items or {} do
		if item.section == "favorite" then
			seen += 1
			if seen == n then
				return { kind = item.kind, id = item.id }
			end
		end
	end
	return nil
end

-- What the head's button calls the list that is shown.
function lists.name()
	for _, own in OWN do
		if own.key == S.scope then
			return own.title
		end
	end
	local key = lists.current()
	for _, item in S.catalog ~= nil and S.catalog.items or {} do
		if item.key == key then
			return item.title
		end
	end
	-- (A project gone to from the list of projects, which the catalog need not hold: the name it had there.)
	if L.names[key] ~= nil then
		return L.names[key]
	end
	return KINDS[S.scope] or "A list"
end

-- The name of the list of `kind` and `id`, where the page knows it from elsewhere than the catalog.
function lists.remember(kind, id, title)
	L.names[kind .. ":" .. id] = title
end

function lists.is_open()
	return L.open
end

-- Opens the card, the cursor on the list that is shown. A card of its own each time: the host keeps
-- the text of a field of the same name, and one opened again would begin with the words of the last.
function lists.show()
	L.serial += 1
	L.state = { query = "", cursor = lists.current(), field = { value = "" } }
	L.open = true
end

function lists.hide()
	L.open = false
end

function lists.view(root)
	if not L.open then
		return
	end
	local shown = items()
	root:child(u:palette("lists" .. L.serial, {
		items = shown,
		version = L.version,
		state = L.state,
		label = "Lists",
		hint = "Go to a view, a project, a team or a cycle",
		rows = kit.card_rows(),
		footer = {
			{ keys = "enter", title = "Open" },
			{ keys = "escape", title = "Close" },
		},
	}))
end

-- What the person did to the card. Whether it was the card's, and the reply.
function lists.action(action)
	if type(action.from) ~= "string" or string.match(action.from, "^lists%d+$") == nil then
		return nil
	end
	if action.kind == "dismiss" then
		L.open = false
	elseif action.kind == "query" then
		L.state.cursor = nil
	elseif action.kind == "pick" then
		-- (The line that says the names have not come goes nowhere: the card stays.)
		if action.item.list == nil then
			return true
		end
		L.open = false
		act.view({ list = action.item.list })
	end
	return true
end

return lists
