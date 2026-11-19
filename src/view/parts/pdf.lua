-- The words of a PDF that is large: selected with the pointer or with Shift and the arrows, copied, and
-- looked for. The terminal draws the page (`image(name):page(n)`); the program reads the same file for
-- what the terminal does not say, and the page asks it (`pdf-open`: how many pages and how large each
-- is; `pdf-text`: the lines of a page with the place of every letter; `pdf-find`: where words are in the
-- whole of it). What is selected is worked out here, on the page's own lines, so that nothing goes to the
-- program as the pointer moves. (The reading of the words and the way a selection is made are textpen's,
-- `view/parts/pdf.lua`, of a view that shows one page at a time.)
--
-- Everything here is of one PDF at a time, the one that is large: its state is `big`, which
-- `pictures.lua` holds (the pages, which one is shown) and gives to each function.

local kit = require("../lib/kit")
local commands = require("./commands")

local b, u, S, SIZE = kit.b, kit.u, kit.S, kit.SIZE
local layout = b.layout

local pdf = {}

local MATCH = b.alpha("warning", 0.38)
local CURRENT = b.alpha("warning", 0.55)
local CURRENT_EDGE = b.mix("warning", "#000000", 0.4)
local SELECTION = b.alpha("accent", 0.3)

-- The words looked for are the field's, whatever the PDF: one field, for the one PDF that is large.
local FIND = { value = "" }
local FIELD = u:name("pdf-find", "field")

-- --- The PDF the program reads --------------------------------------------------------------------------------

-- Asks the program to read the PDF (`pdf-open`); `turn(n)` is what turns the page to `n` (a match found
-- on another page). Until it answers the page knows what the card said (how many pages, and the size of
-- the first), and no words.
function pdf.begin(big, turn)
	big.turn = turn
	big.read = "opening"
	big.runs = nil
	big.words = {}
	big.asked = {}
	big.sel = nil
	big.drag = nil
	big.find = { open = false, found = nil, calling = nil, current = nil, invalid = false }
	FIND.value = ""
	kit.call("pdf-open", { src = big.src, owner = big.owner }, function(result, err)
		if big.read ~= "opening" then
			return nil
		end
		if err or result == nil then
			-- The pages are drawn all the same, with nothing to select or to find.
			big.read = "failed"
			return true
		end
		big.read = "ready"
		big.runs = result.sizes
		big.pages = result.pages
		big.has_text = result.text ~= false
		pdf.want(big, big.page)
		return true
	end)
end

-- The PDF is put away: what is asked of the program is given up, and it lets the PDF go.
function pdf.finish(big)
	if big.find ~= nil and big.find.calling ~= nil then
		b.cancel(big.find.calling)
		big.find.calling = nil
	end
	for _, id in big.asked or {} do
		b.cancel(id)
	end
	big.asked = {}
	big.read = "closed"
	kit.call("pdf-close", { owner = big.owner }, function()
		return nil
	end)
end

-- Whether there are words to select and to find: the program has read the PDF, and it has some.
function pdf.readable(big)
	return big ~= nil and big.pdf == true and big.read == "ready" and big.has_text == true
end

-- The size of page `n`, points: what the program said (runs of pages of one size), else the first page's.
function pdf.size_of(big, n)
	local from = 0
	for _, run in big.runs or {} do
		if n <= from + run[1] then
			return run[2], run[3]
		end
		from += run[1]
	end
	return big.width, big.height
end

-- --- The words of a page -------------------------------------------------------------------------------------

local function chars_of(words)
	local out = {}
	for _, code in utf8.codes(words) do
		out[#out + 1] = utf8.char(code)
	end
	return out
end

local function prepared(answer)
	local lines = {}
	for _, line in answer.lines or {} do
		line.chars = chars_of(line.text)
		lines[#lines + 1] = line
	end
	return { lines = lines }
end

-- Asks the program for the words of page `n`, unless they are held or on the way.
function pdf.want(big, n)
	if not pdf.readable(big) or big.words[n] ~= nil or big.asked[n] ~= nil then
		return
	end
	big.asked[n] = kit.call("pdf-text", { owner = big.owner, page = n }, function(result, err)
		big.asked[n] = nil
		if big.read ~= "ready" then
			return nil
		end
		big.words[n] = (err or result == nil) and { lines = {} } or prepared(result)
		return true
	end)
end

-- The page is turned: its words are asked for, and what was selected on the page before is not.
function pdf.went_to(big, n)
	big.sel, big.drag = nil, nil
	pdf.want(big, n)
end

local function layer_of(big)
	return big.words ~= nil and big.words[big.page] or nil
end

-- --- Places in the words -------------------------------------------------------------------------------------

local function before(a, z)
	if a.line ~= z.line then
		return a.line < z.line
	end
	return a.k < z.k
end

local function same(a, z)
	return a.line == z.line and a.k == z.k
end

local function ordered(sel)
	if before(sel.head, sel.anchor) then
		return sel.head, sel.anchor
	end
	return sel.anchor, sel.head
end

function pdf.selected(big)
	return big ~= nil and big.sel ~= nil and not same(big.sel.anchor, big.sel.head)
end

-- The place nearest to `px`, `py` (points from the page's top left corner) in the words of the page: the
-- line it is nearest, and how many of that line's characters come before it. Whether the point is on it.
local function place_at(layer, px, py)
	if layer == nil or #layer.lines == 0 then
		return nil, false
	end
	local best, best_gap, on = nil, math.huge, false
	for index, line in layer.lines do
		local dy = 0
		if py < line.y then
			dy = line.y - py
		elseif py > line.y + line.h then
			dy = py - line.y - line.h
		end
		local dx = 0
		if px < line.x then
			dx = line.x - px
		elseif px > line.x + line.w then
			dx = px - line.x - line.w
		end
		local gap = dy * 8 + dx
		if gap < best_gap then
			best, best_gap, on = index, gap, dy == 0 and dx == 0
		end
	end
	local line = layer.lines[best]
	local across = px - line.x
	local k, nearest = 0, math.huge
	for at = 1, #line.xs do
		local distance = math.abs(line.xs[at] - across)
		if distance < nearest then
			k, nearest = at - 1, distance
		end
	end
	return { line = best, k = k }, on
end

-- The kind of a character, for the run a double click takes.
local function kind_of(c)
	if c == " " or c == "\t" or c == "\u{a0}" then
		return "space"
	elseif #c > 1 or string.find(c, "^[%w_]$") then
		return "word"
	end
	return "mark"
end

-- The places that bound the word at `place` (the run of letters, or of marks, or of spaces it is in).
local function word_at(layer, place)
	local line = layer.lines[place.line]
	local count = #line.chars
	if count == 0 then
		return place, place
	end
	local at = math.min(place.k + 1, count)
	local kind = kind_of(line.chars[at])
	local from, to = at, at
	while from > 1 and kind_of(line.chars[from - 1]) == kind do
		from -= 1
	end
	while to < count and kind_of(line.chars[to + 1]) == kind do
		to += 1
	end
	return { line = place.line, k = from - 1 }, { line = place.line, k = to }
end

local function line_at(layer, place)
	local line = layer.lines[place.line]
	return { line = place.line, k = 0 }, { line = place.line, k = #line.chars }
end

-- The parts of the selection's lines: for each line it touches, from which character to which.
local function parts(layer, sel)
	local a, z = ordered(sel)
	local out = {}
	for index = a.line, math.min(z.line, #layer.lines) do
		local line = layer.lines[index]
		local count = #line.chars
		local from = index == a.line and a.k or 0
		local to = index == z.line and math.min(z.k, count) or count
		if to > from then
			out[#out + 1] = { index = index, line = line, from = from, to = to }
		end
	end
	return out
end

-- The words the selection holds, a line to a line.
local function selection_words(big)
	local layer = layer_of(big)
	if layer == nil or not pdf.selected(big) then
		return ""
	end
	local out = {}
	for _, part in parts(layer, big.sel) do
		out[#out + 1] = table.concat(part.line.chars, "", part.from + 1, part.to)
	end
	return table.concat(out, "\n")
end

-- --- The marks on the page ------------------------------------------------------------------------------------

local function mark(rect, sx, sy, ground, id)
	return b.div()
		:id(id)
		:absolute()
		:inset(rect.y * sy, nil, nil, rect.x * sx)
		:size(math.max(rect.w * sx, 1), math.max(rect.h * sy, 1))
		:rounded(2)
		:bg(ground)
end

-- What is laid over the page `n` drawn `w` by `h`: the matches of the search, the one it is at marked
-- more, and what is selected.
function pdf.layer(big, n, w, h)
	local out = {}
	local pw, ph = pdf.size_of(big, n)
	local sx, sy = w / pw, h / ph
	local found = big.find ~= nil and big.find.found or nil
	if found ~= nil then
		for _, at in found.by_page[n] or {} do
			local current = at == found.current
			local box = mark(found.matches[at], sx, sy, current and CURRENT or MATCH, "pdf-match:" .. at)
			if current then
				box:border(1, CURRENT_EDGE)
			end
			out[#out + 1] = box
		end
	end
	local layer = layer_of(big)
	if layer ~= nil and pdf.selected(big) then
		for _, part in parts(layer, big.sel) do
			local line = part.line
			out[#out + 1] = mark({
				x = line.x + line.xs[part.from + 1],
				y = line.y,
				w = line.xs[part.to + 1] - line.xs[part.from + 1],
				h = line.h,
			}, sx, sy, SELECTION, "pdf-selection:" .. part.index)
		end
	end
	return out
end

-- --- The pointer ---------------------------------------------------------------------------------------------

-- The point `x`, `y` of the page as points from the page's top left corner, and whether it is on the page.
local function point_of(big, x, y)
	local r = layout.bounds("pdf-page")
	if r == nil then
		return nil
	end
	local pw, ph = pdf.size_of(big, big.page)
	local on = x >= r.x and x < r.x + r.w and y >= r.y and y < r.y + r.h
	return (x - r.x) / r.w * pw, (y - r.y) / r.h * ph, on
end

local function inside(rect, x, y)
	return rect ~= nil and x >= rect.x and x < rect.x + rect.w and y >= rect.y and y < rect.y + rect.h
end

-- Whether the pointer is over the words of the page (it is a text pointer there).
function pdf.over_text(big, event)
	if not pdf.readable(big) or not inside(layout.bounds("picture-zoom"), event.x, event.y) then
		return false
	end
	local px, py, on = point_of(big, event.x, event.y)
	if px == nil or not on then
		return false
	end
	local _, over = place_at(layer_of(big), px, py)
	return over
end

-- Sets the end of the selection the drag is making to the point `x`, `y`.
local function drag_to(big, x, y)
	local layer = layer_of(big)
	local px, py = point_of(big, x, y)
	if px == nil then
		return false
	end
	local place = place_at(layer, px, py)
	if place == nil then
		return false
	end
	local from, to = place, place
	if big.drag.mode == "word" then
		from, to = word_at(layer, place)
	elseif big.drag.mode == "line" then
		from, to = line_at(layer, place)
	end
	local base = big.drag.base
	if before(from, base.from) then
		big.sel = { anchor = base.to, head = from }
	else
		big.sel = { anchor = base.from, head = to }
	end
	return true
end

-- A press on the page: whether it was the words' own (it starts a selection). The reply, or nil.
function pdf.pressed(big, event)
	if not pdf.readable(big) or (event.button or "left") ~= "left" then
		return nil
	end
	local hit = layout.hit({ x = event.x, y = event.y })
	if hit ~= nil and kit.buttons[hit] then
		return nil
	end
	if not inside(layout.bounds("picture-zoom"), event.x, event.y) then
		-- A press anywhere else (not on a button) lets the selection go: Esc does not.
		if big.sel ~= nil then
			big.sel, big.drag = nil, nil
			return true
		end
		return nil
	end
	-- The keys are the page's again, with the search kept open.
	local reply = u.holder == FIELD and u:focus(nil) or true
	local px, py = point_of(big, event.x, event.y)
	local layer = layer_of(big)
	local place = px ~= nil and place_at(layer, px, py) or nil
	if place == nil then
		big.sel, big.drag = nil, nil
		return reply
	end
	local mods = event.mods or {}
	local count = event.count or 1
	local from, to, mode = place, place, "char"
	if count == 2 then
		from, to = word_at(layer, place)
		mode = "word"
	elseif count >= 3 then
		from, to = line_at(layer, place)
		mode = "line"
	end
	if mods.shift and big.sel ~= nil and count < 2 then
		big.sel = { anchor = big.sel.anchor, head = place }
		big.drag = { mode = "char", base = { from = big.sel.anchor, to = big.sel.anchor } }
	else
		big.sel = { anchor = from, head = to }
		big.drag = { mode = mode, base = { from = from, to = to } }
	end
	return reply
end

-- The pointer moved with the button held: the selection follows it. The reply, or nil.
function pdf.dragged(big, event)
	if big == nil or big.drag == nil then
		return nil
	end
	return drag_to(big, event.x, event.y) and true or nil
end

-- The button went up: the selection is what it was.
function pdf.released(big)
	if big == nil or big.drag == nil then
		return nil
	end
	big.drag = nil
	if big.sel ~= nil and same(big.sel.anchor, big.sel.head) then
		big.sel = nil
	end
	return true
end

-- --- Selecting, copying ----------------------------------------------------------------------------------------

-- Selects all the words of the page.
function pdf.select_all(big)
	local layer = layer_of(big)
	if layer == nil or #layer.lines == 0 then
		return nil
	end
	local last = layer.lines[#layer.lines]
	big.sel = { anchor = { line = 1, k = 0 }, head = { line = #layer.lines, k = #last.chars } }
	return true
end

-- Drops the selection: whether there was one.
function pdf.clear_selection(big)
	if big ~= nil and big.sel ~= nil then
		big.sel, big.drag = nil, nil
		return true
	end
	return false
end

-- Puts the selection on the pasteboard. The reply, or nil.
function pdf.copy(big)
	if not pdf.selected(big) then
		return nil
	end
	local words = selection_words(big)
	local count = 0
	for _ in string.gmatch(words, "%S+") do
		count += 1
	end
	kit.toast(count == 1 and "Copied 1 word" or string.format("Copied %d words", count), "info", 2.4)
	return { copy = words }
end

-- Shift and an arrow: the head of the selection goes a character or a line, from the anchor (which is the
-- start of the first line where nothing is selected yet). The reply, or nil.
function pdf.extend(big, key)
	local layer = layer_of(big)
	if layer == nil or #layer.lines == 0 then
		return nil
	end
	local sel = big.sel or { anchor = { line = 1, k = 0 }, head = { line = 1, k = 0 } }
	local head = { line = sel.head.line, k = sel.head.k }
	local line = layer.lines[head.line]
	if key == "right" then
		if head.k < #line.chars then
			head.k += 1
		elseif head.line < #layer.lines then
			head.line, head.k = head.line + 1, 0
		end
	elseif key == "left" then
		if head.k > 0 then
			head.k -= 1
		elseif head.line > 1 then
			head.line = head.line - 1
			head.k = #layer.lines[head.line].chars
		end
	elseif key == "down" or key == "up" then
		local to = math.max(1, math.min(head.line + (key == "down" and 1 or -1), #layer.lines))
		-- The same place across, as near as the other line's letters stand.
		local across = line.xs[head.k + 1] + line.x
		local target = layer.lines[to]
		local k, nearest = 0, math.huge
		for at = 1, #target.xs do
			local distance = math.abs(target.x + target.xs[at] - across)
			if distance < nearest then
				k, nearest = at - 1, distance
			end
		end
		head.line, head.k = to, k
	else
		return nil
	end
	big.sel = { anchor = sel.anchor, head = head }
	return true
end

-- --- The search ------------------------------------------------------------------------------------------------------

function pdf.find_open(big)
	return big ~= nil and big.find ~= nil and big.find.open
end

function pdf.found(big)
	return big ~= nil and big.find ~= nil and big.find.found ~= nil
end

-- The matches are marked on the pages and the bar is up: the keyboard goes to the field. The reply.
function pdf.open_find(big)
	if not pdf.readable(big) then
		return true
	end
	big.find.open = true
	return u:focus(FIELD)
end

-- The search is over: the marks go. The reply.
function pdf.close_find(big)
	local find = big.find
	if find.calling ~= nil then
		b.cancel(find.calling)
		find.calling = nil
	end
	find.open, find.found, find.invalid = false, nil, false
	FIND.value = ""
	return u:focus(nil)
end

-- Goes to the match at `at`: its page is turned to.
local function go_to(big, at)
	local found = big.find.found
	found.current = at
	local match = found.matches[at]
	if match.page ~= big.page then
		big.turn(match.page)
	end
end

-- Looks for the words of the field in every page; the search before is given up. The first match at or
-- after the page that is shown is the one it comes to.
local function look(big)
	local find = big.find
	if find.calling ~= nil then
		b.cancel(find.calling)
		find.calling = nil
	end
	find.found, find.invalid = nil, false
	local query = FIND.value
	if query == "" then
		return
	end
	find.calling = kit.call("pdf-find", { owner = big.owner, query = query }, function(result, err)
		find.calling = nil
		if err or result == nil then
			find.invalid = true
			return true
		end
		local by_page, first = {}, nil
		for at, match in result.matches do
			by_page[match.page] = by_page[match.page] or {}
			table.insert(by_page[match.page], at)
			if first == nil and match.page >= big.page then
				first = at
			end
		end
		if #result.matches == 0 then
			return true
		end
		find.found = {
			matches = result.matches,
			by_page = by_page,
			current = first or 1,
			truncated = result.truncated == true,
		}
		go_to(big, find.found.current)
		return true
	end)
end

-- The next match (1) or the one before (-1), around the ends. The reply.
function pdf.step(big, direction)
	local found = big.find ~= nil and big.find.found or nil
	if found == nil then
		return true
	end
	go_to(big, (found.current - 1 + direction) % #found.matches + 1)
	return true
end

-- What the field of the search does: words typed look again; Enter goes to the next match (with Shift,
-- the one before); Esc puts the bar away. The reply, or nil.
function pdf.action(big, action)
	if action.from ~= "pdf-find" or big == nil or big.find == nil then
		return nil
	end
	local kind = action.kind
	if kind == "change" then
		FIND.value = action.text
		look(big)
	elseif kind == "submit" then
		if big.find.found == nil and FIND.value ~= "" then
			look(big)
		else
			pdf.step(big, (action.mods ~= nil and action.mods.shift) and -1 or 1)
		end
	elseif kind == "escape" then
		return pdf.close_find(big)
	end
	return true
end

-- How the search stands, in words: which match of how many, that there is none, or that it goes on.
local function count_words(big)
	local find = big.find
	if FIND.value == "" then
		return nil
	end
	if find.invalid then
		return "Could not search"
	end
	if find.calling ~= nil then
		return "Searching"
	end
	if find.found == nil then
		return "No match"
	end
	local total = #find.found.matches
	return string.format("%d of %d%s", find.found.current, total, find.found.truncated and "+" or "")
end

-- The row of the search: the field with how it stands, the way to the match after and the one before, and
-- the way out.
function pdf.find_bar(big)
	local trail = {}
	local words = count_words(big)
	if words ~= nil then
		trail[#trail + 1] = b.text(words):text_size(SIZE.small):color("dim"):no_shrink()
	end
	local found = big.find.found ~= nil
	local row = b.div():id("pdf-find-bar"):row():items_center():gap(8):no_shrink():px(SIZE.pad):pb(10)
	row:child(
		b.div():grow(1):min_w(0):child(u:input("pdf-find", {
			state = FIND,
			label = "Find in the PDF",
			hint = "Find in the PDF",
			trail = trail,
			escape = true,
		}))
	)
	row:child(kit.button("pdf-find-previous", "Previous match", {
		suffix = commands.label("pdf_find_previous"),
		disabled = not found,
		tip = "The match before",
	}))
	row:child(kit.button("pdf-find-next", "Next match", {
		suffix = commands.label("pdf_find_next"),
		disabled = not found,
		tip = "The match after",
	}))
	row:child(kit.button("pdf-find-close", "Done", { suffix = "Esc" }))
	return row
end

-- Whether the keyboard is in the field of the search.
function pdf.typing()
	return u.holder == FIELD
end

return pdf
