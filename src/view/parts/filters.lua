-- The filters of a list: a row of five chips under the head (Status, Priority, Assignee, Labels,
-- Project), each a menu whose picks add and take away values (the kit's picker where the terminal has
-- one: it stays open while the person turns one value after another). They narrow the list in Linear
-- (the program sends them with the list's own filter), whichever list it is, a search too. `f` opens the row
-- with the keyboard on the first chip; Tab goes on, Enter opens the menu, Esc gives the keyboard back
-- to the list. When it is back only what is set stays (a chip with its values, and Clear filters); with
-- none set there is no row. The filters are forgotten when the pen closes (the program holds them, as
-- the page set them). The words of the field are another thing: they narrow what is held, and are
-- the head's.

local icons = require("./icons")
local picker = require("./picker")
local values = require("./values")
local commands = require("./commands")
local kit = require("../lib/kit")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local filters = {}

-- The kinds, in the order the row has them, with what each is called.
local KINDS = {
	{ "status", "Status" },
	{ "priority", "Priority" },
	{ "assignee", "Assignee" },
	{ "labels", "Labels" },
	{ "project", "Project" },
}

-- The priorities in the order the menu offers them (Linear's: the urgent first, none last).
local PRIORITIES = { "urgent", "high", "medium", "low", "none" }

local MENU = "filters-menu"
local PREFIX = "filters:"
local CLEAR = "filters-clear"

-- `picking`: the kind whose picker is open (its chip stays while it is, though it has no value left).
-- `open`: the row is open with all five chips (`f`). `now`: the filters as the page set them (the
-- program holds the same). `options`: what the menus offer, as the program said, and for which asking of
-- Linear (`at`); `failed`: the last asking for them failed.
local F = { open = false, now = nil, options = nil, at = nil, asking = false, failed = false, picking = nil }
filters.state = F

local function empty()
	return { status = {}, priority = {}, assignee = {}, labels = {}, project = {} }
end

-- The filters now: what the page set, else what the program holds.
function filters.current()
	if F.now == nil then
		local held = S.issues ~= nil and S.issues.filters or nil
		F.now = empty()
		if held ~= nil then
			for _, kind in KINDS do
				F.now[kind[1]] = table.clone(held[kind[1]] or {})
			end
		end
	end
	return F.now
end

-- Whether any filter is set.
function filters.any()
	local now = filters.current()
	for _, kind in KINDS do
		if #now[kind[1]] > 0 then
			return true
		end
	end
	return false
end

function filters.is_open()
	return F.open
end

-- Whether the row is on the page: open, or there is something set, or a picker of it is open.
-- (Not in the inbox: Linear's inbox takes no filter, and what the other lists are narrowed by stays theirs.)
function filters.visible()
	return S.scope ~= "inbox" and S.scope ~= "projects" and (F.open or F.picking ~= nil or filters.any())
end

-- The name of the chip of `kind`.
local function chip_name(kind)
	return PREFIX .. kind
end

-- Whether the keyboard is on a chip of the row.
function filters.on_chip()
	local target = u:target()
	return target ~= nil and string.sub(target, 1, #PREFIX) == PREFIX
end

-- --- What the menus offer -----------------------------------------------------------------------------

-- Asks the program for what the menus offer, where it was not asked for the lists as they are now
-- (it lets go of them each time it asks Linear again). A failed asking is made again when the person
-- presses a chip.
local function ask()
	local at = S.issues ~= nil and S.issues.updated or nil
	if F.asking or (F.options ~= nil and F.at == at) then
		return
	end
	F.asking = true
	kit.call("filter_options", {}, function(result, err)
		F.asking = false
		if err then
			F.options, F.failed = nil, true
			kit.lib.toast(S.toasts, "The choices of the filters could not be asked for", "warn")
			return true
		end
		F.options, F.at, F.failed = result, at, false
		return true
	end)
end

-- The choices are asked for once the lists have come, before a chip is pressed: a chip opens on what is
-- here. A failed asking is made again when a chip is pressed, not by itself. As the lists come: not
-- while the page asks for a key or says that Linear did not answer (`loaded` is false), where there is
-- no chip and the asking would fail for the same reason the page already says.
function filters.sync()
	local loaded = S.issues ~= nil and S.issues.loaded == true
	if loaded and F.options == nil and not F.asking and not F.failed then
		ask()
	end
end
act.sync_filters = filters.sync

-- The projects of the lists a person can go to, as `{ id, name }`.
local function projects()
	local out = {}
	for _, item in S.catalog ~= nil and S.catalog.items or {} do
		if item.kind == "project" and item.id ~= nil then
			out[#out + 1] = { id = item.id, name = item.title }
		end
	end
	return out
end

-- The person's own entry among the people.
local function me()
	for _, person in F.options ~= nil and F.options.people or {} do
		if person.me then
			return person
		end
	end
	return nil
end

-- What `value` of `kind` is called.
local function name_of(kind, value)
	if kind == "priority" then
		return values.priority_name(value)
	elseif kind == "assignee" then
		if value == "me" then
			return "Me"
		elseif value == "none" then
			return "No assignee"
		end
		for _, person in F.options ~= nil and F.options.people or {} do
			if person.id == value then
				return person.me and "Me" or person.name
			end
		end
		return "Someone"
	elseif kind == "project" then
		if value == "none" then
			return "No project"
		end
		for _, project in projects() do
			if project.id == value then
				return project.name
			end
		end
		return "A project"
	end
	return value
end

-- Whether `value` is among those set of `kind`.
local function has(kind, value)
	for _, held in filters.current()[kind] do
		if held == value then
			return true
		end
	end
	return false
end

-- The choices of the menu of `kind`, each `{ value, words }`, or nil where they have not come.
local function choices(kind)
	local out = {}
	if kind == "priority" then
		for _, name in PRIORITIES do
			out[#out + 1] = { name, name == "none" and "No priority" or values.priority_name(name) }
		end
	elseif kind == "project" then
		for _, project in projects() do
			out[#out + 1] = { project.id, project.name }
		end
		out[#out + 1] = { "none", "No project" }
	elseif F.options == nil then
		return nil
	elseif kind == "status" then
		-- Every list holds the issues of every state: each state is offered.
		for _, state in F.options.states do
			out[#out + 1] = { state.name, state.name }
		end
	elseif kind == "assignee" then
		-- The person is "Me" whoever they are called, and the others are by name.
		if me() ~= nil then
			out[#out + 1] = { "me", "Me" }
		end
		for _, person in F.options.people do
			if not person.me then
				out[#out + 1] = { person.id, person.name }
			end
		end
		out[#out + 1] = { "none", "No assignee" }
	elseif kind == "labels" then
		for _, label in F.options.labels do
			out[#out + 1] = { label.name, label.name }
		end
	end
	return out
end

-- --- Setting ------------------------------------------------------------------------------------------

-- Sends the filters to the program: only the kinds that have a value (a kind with none is not sent).
local function send(now)
	local out = {}
	for _, kind in KINDS do
		if #now[kind[1]] > 0 then
			out[kind[1]] = now[kind[1]]
		end
	end
	act.view({ filters = out })
end

-- Adds `value` to the filter of `kind`, or takes it away where it is set. The reply.
local function toggle(kind, value)
	local now = filters.current()
	local kept, found = {}, false
	for _, held in now[kind] do
		if held == value then
			found = true
		else
			kept[#kept + 1] = held
		end
	end
	if not found then
		kept[#kept + 1] = value
	end
	now[kind] = kept
	send(now)
	-- (A chip that was set by a press of the pointer and has no value left is gone with its row.)
	if not filters.visible() then
		return u:focus(nil)
	end
	return true
end

-- The program did not take the filters the page sent (it says why): what the chips show is what it holds.
act.filters_refused = function(err)
	F.now = nil
	kit.lib.toast(S.toasts, "The filters were not taken: " .. (err.message or err.code or "no reason given"), "warn")
end

-- Forgets the filters here: the program is told in the same call that goes to another list.
function filters.forget()
	F.now = empty()
	F.open = false
end

-- Lets go of every filter, and gives the keyboard back to the list.
function filters.clear()
	F.now = empty()
	F.open = false
	send(F.now)
	return u:focus(nil)
end

-- --- The row --------------------------------------------------------------------------------------------

-- The most of a name a chip shows; a longer one is cut with the kit's mark.
local NAME_MOST = 20

local function short(text)
	if utf8.len(text) > NAME_MOST then
		return string.sub(text, 1, utf8.offset(text, NAME_MOST) - 1) .. "\u{2026}"
	end
	return text
end

-- What a chip says of its values: the first, and how many more there are.
local function words_of(kind, held)
	local first = short(name_of(kind, held[1]))
	if #held > 1 then
		return first .. " +" .. (#held - 1)
	end
	return first
end

local function words(text, unset)
	return b.text(text):text_size(SIZE.small):color(unset and "dim" or "foreground"):no_shrink()
end

-- What stands before the words of a chip: the picture of what it filters by.
local function picture(kind, held)
	if kind == "status" then
		local shown = "other"
		for _, state in F.options ~= nil and F.options.states or {} do
			if held[1] ~= nil and state.name == held[1] then
				shown = state.kind
			end
		end
		return icons.state(shown)
	elseif kind == "priority" then
		return icons.priority(held[1] or "none")
	elseif kind == "assignee" then
		return icons.person()
	elseif kind == "labels" then
		if held[1] ~= nil then
			for _, label in F.options ~= nil and F.options.labels or {} do
				if label.name == held[1] then
					return icons.dot(label.color)
				end
			end
		end
		return icons.tag()
	end
	return icons.cube()
end

local function chip(kind, title)
	-- (While its card is open the chip says what it held when it opened: it does not move the card.)
	local held = picker.steady(chip_name(kind), filters.current()[kind])
	local kids = { picture(kind, held) }
	if #held > 0 then
		kids[#kids + 1] = words(words_of(kind, held), false)
	else
		kids[#kids + 1] = words(title, true)
	end
	-- (The ring is drawn where the keyboard is: a press of the pointer does not leave it on a chip.)
	local named = {}
	for _, one in held do
		named[#named + 1] = name_of(kind, one)
	end
	return values.chip(chip_name(kind), kids, title, {
		ring = u:target() == chip_name(kind),
		value = table.concat(named, ", "),
	})
end

-- The row under the head, or nil where there is none (nothing is set, and it is not open). A row of
-- its own: the room it takes is the head's, whatever the pane is doing.
function filters.view()
	if not filters.visible() then
		return nil
	end
	local now = filters.current()
	local row = b.div():id("filters-row"):row():wrap():items_center():gap_x(8):gap_y(8):px(SIZE.pad):pb(8):no_shrink()
	for _, kind in KINDS do
		if F.open or #now[kind[1]] > 0 or F.picking == kind[1] then
			row:child(chip(kind[1], kind[2]))
		end
	end
	if filters.any() then
		row:child(kit.button(CLEAR, "Clear filters", {
			suffix = commands.label("clear_filters"),
			tip = "Let go of every filter",
		}))
	end
	return row
end

-- --- What the person does -------------------------------------------------------------------------------

-- Opens the row, the keyboard on its first chip, and asks for what the menus offer. The reply.
function filters.show()
	F.open = true
	ask()
	return u:focus(chip_name(KINDS[1][1]))
end

-- Puts the row away: what is set stays, as chips.
function filters.close()
	F.open = false
end

-- Whether a press that went down on `hit` puts the row away: one that is not on the row (nor in the
-- card of a chip) does, when its button comes up (a menu is not drawn by the page: its entries are not
-- pressed with the page's pointer).
function filters.outside(hit)
	if not F.open or picker.holds(hit) then
		return false
	end
	return hit == nil or (string.sub(hit, 1, #PREFIX) ~= PREFIX and hit ~= CLEAR)
end

-- What stands before a value in the picker of `kind`: the picture of a status, a priority or a label.
local function picture_of(kind, value)
	if kind == "status" then
		for _, state in F.options ~= nil and F.options.states or {} do
			if state.name == value then
				return icons.state(state.kind)
			end
		end
	elseif kind == "priority" then
		return icons.priority(value)
	elseif kind == "labels" then
		for _, label in F.options ~= nil and F.options.labels or {} do
			if label.name == value then
				return icons.dot(label.color)
			end
		end
	end
	return nil
end

-- What the picker of `kind` offers now, each on where it is set; nil where the choices are gone.
local function picker_items(kind)
	local offered = choices(kind)
	if offered == nil then
		-- (Choices on their way: the card says so, and fills when they come. Where the asking failed, it
		-- goes.)
		return F.asking and {} or nil
	end
	local out = {}
	for _, choice in offered do
		out[#out + 1] = { key = choice[1], title = choice[2], icon = picture_of(kind, choice[1]), on = has(kind, choice[1]) }
	end
	return out
end

-- The words of a kind (`labels`: Labels).
local function title_of(kind)
	for _, known in KINDS do
		if known[1] == kind then
			return known[2]
		end
	end
	return kind
end

-- Opens the menu of `kind`, under its chip (the kit's picker, where the terminal has one). The reply.
local function open_menu(kind)
	ask()
	local offered = choices(kind)
	-- (A chip pressed before its choices have come opens its card at once: it says they are coming.)
	if picker.available() and ((offered == nil and F.asking) or (offered ~= nil and #offered > 0)) then
		local title = title_of(kind)
		local opened = picker.open({
			anchor = chip_name(kind),
			label = title,
			hint = "Filter by " .. string.lower(title),
			empty = function()
				return choices(kind) == nil and "Loading" or nil
			end,
			items = function()
				return picker_items(kind)
			end,
			toggle = function(item)
				return toggle(kind, item.key)
			end,
			closed = function()
				F.picking = nil
			end,
		})
		if opened then
			F.picking = picker.is_open() and kind or nil
			return true
		end
	end
	if offered == nil or #offered == 0 then
		-- (The host's menu opens on what is here: pressed before the choices have come, it only asks.)
		return true
	end
	local entries = {}
	for _, choice in offered do
		entries[#entries + 1] = {
			key = kind .. ":" .. choice[1],
			title = values.mark(has(kind, choice[1]), choice[2]),
		}
	end
	return u:menu(MENU, entries, { near = chip_name(kind) }) or true
end

-- A chip or Clear filters pressed. The reply, or nil where it was not the row's.
function filters.press(id)
	if id == CLEAR then
		return filters.clear()
	elseif string.sub(id, 1, #PREFIX) == PREFIX then
		return open_menu(string.sub(id, #PREFIX + 1))
	end
	return nil
end

-- What a menu of the row did. The reply, or nil where it was not the row's.
function filters.action(action)
	if action.from ~= MENU or action.kind ~= "menu" then
		return nil
	end
	local at = string.find(action.entry, ":", 1, true)
	if at == nil then
		return true
	end
	return toggle(string.sub(action.entry, 1, at - 1), string.sub(action.entry, at + 1))
end

return filters
