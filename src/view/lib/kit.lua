-- What every file of the page shares: the kit's builders, the library's context `u` (which remembers
-- the pointer and the keyboard and gives the components), the page's state, the reply that what the
-- page does is gathered into, and the few pieces the page is made of more than once (a button that
-- carries its key, an icon).

local keys = require("@pencere/keys")
local lib = require("@pencere/ui")
local SIZE = require("./sizes")

local b = pencere
-- Compact: a list of issues is a dense page.
local u = lib.new({ density = "compact" })

-- The page's state: what the program said and what the person did, in one place, so that `render` is
-- a function of it.
local S = {
	now = 0,
	width = 1000,
	height = 700,
	-- The room the list has: the page's width, less what an issue beside it takes.
	room = 1000,
	-- An issue, or the hand-over to an agent, is open beside the list (or over it).
	panel_open = false,
	-- The hand-over is the one open.
	work_open = false,
	-- What `boot` said, and why there is none.
	boot = nil,
	boot_error = nil,
	-- What the topic `issues` last said (`docs/calls.md`), and which list the person is looking at.
	issues = nil,
	scope = "mine",
	-- The groups the person folded or opened, by key; sent with every `view`.
	closed = {},
	-- The row the cursor is on: `{ kind = "issue", id = }` or `{ kind = "group", key = }`.
	cursor = nil,
	-- The Connect screen: the key typed, an answer awaited, why the last was refused.
	connecting = false,
	connect_error = nil,
	-- Calls out that the person waits on.
	refreshing = false,
	-- What is said for a moment (a copy was made): the library's toasts.
	toasts = {},
}

-- The sizes (lib/sizes.lua) and the theme's.
local SIZES = SIZE

-- --- What the page does is gathered into one reply ----------------------------------------------

local gathered = nil

local function gather(reply)
	gathered = b.merge(gathered, reply)
end

-- The reply of a way into the page: `reply` with what was gathered.
local function take(reply)
	local out = gathered
	gathered = nil
	return b.merge(out, reply)
end

-- Asks the program to do `method`; `done(result, err)` is called with its answer, and what it
-- gathered goes out with that answer's reply. `err` is { code =, message = }.
local function call(method, params, done)
	return b.call(method, params, function(result, err)
		local reply = nil
		if done then
			reply = done(result, err)
		end
		return take(reply == nil and true or reply)
	end)
end

-- --- Words and pieces ---------------------------------------------------------------------------

-- One of the pictures of src/assets/icons (Tabler's, MIT), `size` points, in a colour of the theme.
local function icon(name, size, color)
	size = size or 15
	return b.image("assets/icons/" .. name .. ".svg"):size(size, size):tint(color or "dim"):no_shrink()
end

-- A picture turning round, for what is being waited for: the host turns it, a frame at a time.
local function spinner(size, color)
	return icon("loader-2", size, color):motion({ spin = 1 })
end

-- The keys of a command as the host's keyboard writes them (`@pencere/keys`).
local function keys_of(spec)
	if type(spec) == "table" then
		spec = spec[1]
	end
	if not spec then
		return nil
	end
	return keys.label(spec)
end

-- The names of the buttons drawn in this render (`button`), so that a press on one is told from a
-- press on anything else.
local buttons = {}

-- A button: a verb, the key that does the same dim inside it. `o`: `primary`, `disabled`, `suffix`
-- (the key, as it is written), `tip`, `busy` (a turning picture before the words), `small`, `bar` (the
-- size of the buttons in the bar of the selection, which is as high as the line of keys), `bold`, `count` (a
-- number after the words, dim) and `chevron` (the mark of a button that opens a card). It is
-- named `id` and pressed with the pointer, or with enter or the space bar while it has the keyboard
-- (`page.lua`'s `on_button`). A disabled one answers nothing and says why in its tip.
local function button(id, label, o)
	o = o or {}
	local t = u.theme
	buttons[id] = not o.disabled and true or nil
	local over = u:hovered(id) and not o.disabled
	local held = over and u:pressed(id)
	local ground, ink
	if o.primary then
		ground = held and b.mix("accent", "foreground", 0.2) or over and b.mix("accent", "foreground", 0.12) or "accent"
		ink = "background"
	else
		ground = held and t.control_press or over and t.control_hover or t.control
		ink = t.ink
	end
	local el = b.div()
		:row()
		:items_center()
		:justify_center()
		:gap(o.bar and 6 or 9)
		:h(o.bar and SIZE.bar_control or o.small and SIZE.small_control or t.size.control)
		:px(o.bar and 7 or o.small and 8 or t.size.pad)
		:no_shrink()
		:rounded(t.radius.control)
		:bg(ground)
		:border(1, u:focus_visible(id) and t.ring or t.clear)
		:color(ink)
		:role("button")
		:label(label)
		:motion({ seconds = t.motion, color = true })
	if not o.disabled then
		el:press(id):focusable()
	end
	if o.busy then
		el:child(spinner(13, ink))
	end
	local words = b.text(label):weight(o.bold and 700 or o.primary and 600 or 500)
	if o.bar then
		words = words:text_size(SIZE.tiny + 0.5)
	end
	el:child(words)
	if o.count then
		el:child(b.text(o.count):text_size(SIZE.small):color(b.alpha(ink, 0.6)):no_shrink())
	end
	if o.chevron then
		el:child(icon("chevron-down", 13, b.alpha(ink, 0.7)))
	end
	if o.suffix then
		el:child(b.text(o.suffix):text_size(SIZE.tiny):color(b.alpha(ink, 0.6)))
	end
	if o.tip then
		el:tip(o.tip)
	end
	if o.disabled then
		el:disabled(true):opacity(0.45)
	end
	return el
end

-- The actions of the page, by name, filled by the parts that do them: a part calls another's action
-- through here, so that no two parts require each other.
local act = {}

return {
	b = b,
	u = u,
	lib = lib,
	S = S,
	SIZE = SIZES,
	act = act,
	gather = gather,
	take = take,
	call = call,
	icon = icon,
	keys_of = keys_of,
	spinner = spinner,
	button = button,
	buttons = buttons,
	scroll_to = function(id, y, animate)
		b.layout.scroll_to(id, { y = math.max(y, 0), animate = animate == true })
	end,
	offset = function(id)
		local y = b.layout.scroll_offset(id)
		return y
	end,
}
