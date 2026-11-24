-- The files of Linear in the text of an issue: a picture (`![alt](address)`) or a link to one of Linear's
-- files that stands alone in its paragraph (`[name](address)`), in its description and its comments.
-- The program fetches each (up to 8 MiB) with the key and gives it to the terminal (`picture`). A PNG or
-- a JPEG is drawn in its place, as wide as the room and never larger, and a press opens it large over the
-- pane. A PDF is a card whose one verb is Open: it opens it, page by page, over the pane. Any other file
-- (a GIF, a log, a zip, a video, one that is too large to show) is a card too: its icon, its name, what it
-- is and why it is not drawn, and one verb, Save (the person chooses the place and the program writes the
-- file there). A card is pressed as a whole, and what else is done to it (the PDF's Save, Open the link,
-- Copy the file's link) is in its right-click menu and in the commands. A picture of an address that is not
-- Linear's is a link that opens in the browser (it is never fetched). The files are held for the issue
-- that is open, and let go when it is not (`release`).

local kit = require("../lib/kit")
local commands = require("./commands")
local kinds = require("./kinds")
local pdf = require("./pdf")
local markdown = require("@pencere/markdown")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local pictures = {}

-- Whether the terminal draws a page of a PDF: the kit that does gave the elements `page`
-- (`Element:page`), an older terminal's have none. A test of an older terminal sets it false.
pictures.draws_pdf = b.div().page ~= nil

-- Where the files of Linear are: the only address the page asks the program about.
local UPLOADS = "https://uploads.linear.app/"

-- The tallest a picture is in the text, points.
local TALL = 520

-- The most files asked for an issue.
local MOST = 40

-- What the page holds: the issue the files are held for (`owner`), what the program said of each of
-- the issue's files by address (`answers[owner][src]`: `{ status = "loading" | "ready" | "refused",
-- name, width, height, pdf, pages, why }`), the texts that were looked at for addresses (so that one is
-- looked at once), the file that is large (`big`, with the page a PDF is at), what a file of this frame is
-- (`ids`, by the name its element has) and which cards there are (`cards`, in the order they are drawn:
-- they are made as the frame is drawn), the place the person was asked to save in (`save`, until the
-- terminal tells the handle of it) and the addresses being saved (`saving`).
local Pc = {
	owner = nil,
	answers = {},
	scanned = {},
	trees = {},
	big = nil,
	ids = {},
	cards = {},
	seen = {},
	count = 0,
	scroll = nil,
	save = nil,
	saving = {},
}
pictures.state = Pc

-- What the title of a picture says where the page made it of a link that stands alone (a file of Linear
-- that was written as a link): a picture written as one has the title the person gave it.
local LINKED = "\0linked"

-- The kinds of picture the program names, as they are said.
local KINDS = { gif = "GIF", webp = "WebP", png = "PNG", jpg = "JPEG" }

-- Why a file is not drawn, in words: where it is too large, how large; what the bytes say it is, where
-- they say a picture that is not drawn; and that it was not fetched. A picture written as one (`image`)
-- always has a reason; a file written as a link needs none (its card says what it is).
local function reasons(answer, image)
	local out = {}
	local why = answer.why
	if why == "too-large" then
		if answer.width ~= nil and answer.height ~= nil then
			out[#out + 1] = answer.width .. "\u{d7}" .. answer.height
		end
		out[#out + 1] = "Too large to show here"
	elseif why == "unreachable" then
		out[#out + 1] = "Could not be fetched"
	elseif why == "format" then
		local kind = KINDS[answer.kind]
		if kind ~= nil then
			out[#out + 1] = kind .. " is not shown here"
		elseif image then
			out[#out + 1] = "Not shown here"
		end
	end
	return out
end

-- The name of a file: the words of its text, else the last part of its address, else `fallback` (`image`).
local function name_of(src, alt, fallback)
	if alt ~= nil and alt ~= "" then
		return alt
	end
	local path = string.match(src or "", "^[^?#]*") or ""
	local last = string.match(path, "([^/]+)$")
	return (last ~= nil and last ~= "") and last or (fallback or "image")
end

-- The scale a picture of `width` by `height` is drawn at in a room of `room_w` by `room_h`: smaller in
-- its proportions where the room is smaller, never larger (1).
local function fit(width, height, room_w, room_h)
	return math.min(1, math.max(room_w, 1) / width, math.max(room_h, 1) / height)
end

local function fitted(width, height, room_w, room_h)
	local scale = fit(width, height, room_w, room_h)
	return math.max(1, math.floor(width * scale + 0.5)), math.max(1, math.floor(height * scale + 0.5))
end

-- --- What the program is asked --------------------------------------------------------------------------

local function ask(owner, mine, src)
	if string.sub(src, 1, #UPLOADS) ~= UPLOADS then
		-- Not Linear's: it is never asked for, with the key or without.
		mine[src] = { status = "refused", why = "not-linear" }
		return
	end
	mine[src] = { status = "loading" }
	kit.call("picture", { src = src, owner = owner }, function(result, err)
		local still = Pc.answers[owner] == mine
		if err then
			mine[src] = { status = "refused", why = "unreachable" }
		elseif result.refused ~= nil then
			mine[src] = {
				status = "refused",
				why = result.refused,
				kind = result.kind,
				width = result.width,
				height = result.height,
			}
		else
			mine[src] = {
				status = "ready",
				name = result.name,
				width = result.width,
				height = result.height,
				pdf = result.pdf == true,
				pages = result.pages,
			}
		end
		if not still and Pc.owner ~= owner then
			-- The issue went while it was asked: the program holds it for no one the page knows.
			kit.call("release", { owner = owner }, function() end)
		end
		return true
	end)
end

-- Whether `node` is a text of only white space.
local function blank(node)
	return node.type == "softbreak" or (node.type == "text" and string.match(node.text or "", "^%s*$") ~= nil)
end

-- The words of inlines, without their looks.
local function plain(nodes)
	local out = {}
	for _, node in nodes or {} do
		if node.type == "text" or node.type == "code" then
			out[#out + 1] = node.text or ""
		elseif node.children ~= nil then
			out[#out + 1] = plain(node.children)
		end
	end
	return table.concat(out)
end

-- A link to one of Linear's files that stands alone in its paragraph is a file there as a picture is: the
-- paragraph becomes the picture of its address (`link` says it was a link). Done in the tree, so that the
-- text of Linear's is never searched for words, and a link in a sentence stays a link.
local function standalone(node, depth)
	if depth > 40 then
		return
	end
	if node.type == "paragraph" and node.children ~= nil then
		local only = nil
		for _, child in node.children do
			if not blank(child) then
				if only ~= nil or child.type ~= "link" then
					only = nil
					break
				end
				only = child
			end
		end
		if only ~= nil and string.sub(only.href or "", 1, #UPLOADS) == UPLOADS then
			node.children = {
				{ type = "image", src = only.href, alt = plain(only.children), title = LINKED, link = true },
			}
		end
	end
	for _, value in node do
		if type(value) == "table" then
			standalone(value, depth + 1)
		end
	end
end

-- The tree of `text` (markdown) with the files that stand alone made pictures; nil where it does not
-- parse. Kept for the text, so that it is read once.
function pictures.tree(text)
	local kept = Pc.trees[text]
	if kept ~= nil then
		return kept or nil
	end
	local ok, tree = pcall(markdown.parse, text)
	if not ok then
		Pc.trees[text] = false
		return nil
	end
	standalone(tree, 0)
	Pc.trees[text] = tree
	return tree
end

-- The pictures of a tree, in the order they stand.
local function images_of(node, out, depth)
	if depth > 40 then
		return out
	end
	if node.type == "image" and node.src ~= nil then
		out[#out + 1] = node
	end
	for _, value in node do
		if type(value) == "table" then
			images_of(value, out, depth + 1)
		end
	end
	return out
end

-- Asks for the files of `detail`, the issue `owner`: those of its description and its comments, once.
-- A link that stands alone is asked for only where its name says a picture or a PDF (any other is a card
-- without a download that would only be thrown away).
local function want(owner, detail)
	if Pc.scanned[owner] == detail then
		return
	end
	Pc.scanned[owner] = detail
	local mine = Pc.answers[owner]
	if mine == nil then
		mine = {}
		Pc.answers[owner] = mine
	end
	local texts = { detail.description or "" }
	for _, one in detail.comments or {} do
		texts[#texts + 1] = one.body or ""
	end
	local asked = 0
	for _, text in texts do
		local tree = pictures.tree(text)
		for _, node in tree ~= nil and images_of(tree, {}, 0) or {} do
			local src = node.src
			if mine[src] == nil and asked < MOST then
				if node.link and not (kinds.drawable_name(node.alt) or kinds.drawable_name(src)) then
					mine[src] = { status = "refused", why = "format" }
				else
					asked += 1
					ask(owner, mine, src)
				end
			end
		end
	end
end

-- The issue the page shows is `owner` (nil: none), with what the program told of it, `detail` (nil: not
-- yet). What was held for the issue before is let go; what the new one has is asked for.
function pictures.track(owner, detail)
	if Pc.owner ~= owner then
		local before = Pc.owner
		Pc.owner = owner
		Pc.pointed = nil
		if before ~= nil then
			Pc.answers[before] = nil
			Pc.scanned[before] = nil
			Pc.trees = {}
			kit.call("release", { owner = before }, function() end)
		end
		if Pc.big ~= nil and Pc.big.owner ~= owner then
			Pc.big = nil
		end
	end
	if owner ~= nil and detail ~= nil then
		want(owner, detail)
	end
end

-- --- In the text ----------------------------------------------------------------------------------------

-- A frame is drawn: the files are numbered again from the first.
function pictures.begin()
	Pc.count = 0
	Pc.ids = {}
	Pc.seen = Pc.cards
	Pc.cards = {}
end

-- What a card of a file is called: the words of the text, else the name in the address.
local function card_name(src, alt)
	return name_of(src, alt, "file")
end

-- The card of `cards` the file keys are for: the one whose verb has the keyboard, else the one the
-- right-click menu was opened on (until a press elsewhere), else the only one there is. Nil for none.
local function target_in(cards)
	for _, one in cards do
		if u:focused(one.ids.verb) then
			return one
		end
	end
	if Pc.pointed ~= nil then
		for _, one in cards do
			if one.src == Pc.pointed then
				return one
			end
		end
	end
	if #cards == 1 then
		return cards[1]
	end
	return nil
end

-- Whether the card `n` of the frame before is the one the file keys are for: its verb carries the key of
-- the verb.
local function keyed(n)
	local one = target_in(Pc.seen)
	return one ~= nil and one.n == n
end

-- The card of a file that is not drawn in the text (a PDF, a GIF, a log, a picture too large to show): its
-- icon, its name, what it is and why it is not drawn, and the one verb: Open for a PDF, Save for any other.
-- Cards are of one width (the room, at most 480 points). The card is pressed as a whole for that verb; the rest (Save of a PDF, Open the link, Copy the file's
-- link) is its right-click menu. `answer` is what the program said of it (nil where it was never asked: a
-- link whose name says no picture), `image` that it was written as a picture.
local function card(n, owner, src, alt, answer, image)
	local t = u.theme
	local name = card_name(src, alt)
	local is_pdf = answer ~= nil and answer.pdf == true
	local one = { owner = owner, src = src, alt = alt }
	local ids = { card = "file-card:" .. n, save = "file-save:" .. n, link = "pic-link:" .. n }
	Pc.ids[ids.save] = one
	Pc.ids[ids.link] = one
	Pc.ids[ids.card] = one
	if is_pdf and pictures.draws_pdf then
		ids.open = "file-open:" .. n
		Pc.ids[ids.open] = one
	end
	ids.verb = ids.open or ids.save
	Pc.cards[#Pc.cards + 1] = { n = n, src = src, ids = ids, pdf = ids.open ~= nil, kind = "file" }
	kit.buttons[ids.card] = true
	local meta = { is_pdf and "PDF" or image and "Image" or kinds.label(name) }
	if is_pdf and answer.pages ~= nil then
		meta[#meta + 1] = answer.pages == 1 and "1 page" or (answer.pages .. " pages")
	end
	if answer ~= nil and answer.status == "refused" then
		for _, words in reasons(answer, image) do
			meta[#meta + 1] = words
		end
	end
	local saving = Pc.saving[src] == true
	local icon = is_pdf and "file-type-pdf" or image and "photo" or kinds.icon(name, nil)
	local top = b.div():row():items_center():gap(10):min_w(0):grow(1)
	top:child(
		b.div()
			:size(32, 32)
			:no_shrink()
			:rounded(6)
			:bg(b.alpha("foreground", 0.06))
			:items_center()
			:justify_center()
			:child(kit.icon(icon, 20, "dim"))
	)
	top:child(
		b.div():col():gap(2):grow(1):min_w(0):child(b.text(name):text_size(SIZE.small):wrap()):child(
			b.text(table.concat(meta, "  ")):text_size(SIZE.tiny):color("dim"):wrap()
		)
	)
	local verb
	if ids.open ~= nil then
		verb = kit.button(ids.open, "Open", {
			small = true,
			suffix = keyed(n) and commands.label("file_open") or nil,
			tip = "Read the PDF, page by page",
		})
	else
		verb = kit.button(ids.save, "Save", {
			small = true,
			suffix = keyed(n) and commands.label("file_save") or nil,
			busy = saving,
			disabled = saving,
			tip = "Save " .. name .. " in a place you choose",
		})
	end
	return b.div()
		:press(ids.card)
		:row()
		:items_center()
		:gap(10)
		:p(10)
		:rounded(t.radius.control)
		:border(1, t.line)
		:bg(u:hovered(ids.card) and b.alpha("foreground", 0.04) or t.clear)
		:w_full()
		:max_w(480)
		:tip(ids.open ~= nil and "Read the PDF, page by page" or ("Save " .. name))
		:child(top)
		:child(b.div():no_shrink():child(verb))
end

-- Where an address is the web's (`http` or `https`): the only kind that is opened in the browser.
local function is_web(src)
	return string.match(string.lower(string.sub(src or "", 1, 8)), "^https?://") ~= nil
end

-- The host of a web address, for what a link of no words is called.
local function host_of(src)
	local rest = string.match(src, "^%a+://([^/?#]+)")
	rest = rest ~= nil and string.match(rest, "([^@]*)$") or nil
	return (rest ~= nil and rest ~= "") and rest or nil
end

-- A picture of an address that is not Linear's: it is never fetched, with the key or without, and not
-- drawn. It is a link in the colour of links: its words (else the host), focusable, pressed (or Enter) it
-- opens in the browser; the tip says the whole address.
local function outside(n, owner, src, alt)
	local t = u.theme
	local id = "pic-ext:" .. n
	Pc.ids[id] = { owner = owner, src = src, alt = alt }
	Pc.cards[#Pc.cards + 1] = { n = n, src = src, ids = { verb = id, link = id, card = id }, kind = "link" }
	kit.buttons[id] = true
	local words = (alt ~= nil and alt ~= "") and alt or host_of(src) or name_of(src, alt)
	return b.div()
		:press(id)
		:focusable()
		:role("link")
		:label(words)
		:tip(src)
		:min_w(0)
		:rounded(t.radius.control)
		:border(1, u:focus_visible(id) and t.ring or t.clear)
		:child(b.text(words):color("accent"):underline():wrap())
end

-- What is drawn where the file was refused: where it was not fetched at all (an address that is not
-- Linear's, a terminal that holds no files) the link or the words, else its card.
local function refused(n, owner, src, alt, answer, image)
	local why = answer.why
	if why == "not-linear" and is_web(src) then
		return outside(n, owner, src, alt)
	elseif why == "no-view" or why == "not-linear" then
		-- The words of the picture, as they were drawn before the pictures were (an address that is not
		-- the web's is not made a link).
		return b.text(name_of(src, alt)):color("dim"):wrap()
	end
	return card(n, owner, src, alt, answer, image)
end

local function drawn(n, owner, src, alt, answer, room)
	local t = u.theme
	local w, h = fitted(answer.width, answer.height, room, TALL)
	local id = "pic:" .. n
	Pc.ids[id] = { owner = owner, src = src, alt = alt }
	kit.buttons[id] = true
	local label = name_of(src, alt)
	local shown = b.image(answer.name):size(w, h):fit("contain"):no_shrink()
	return b.div()
		:press(id)
		:focusable()
		:role("image")
		:label(label)
		:tip("View it larger")
		:no_shrink()
		:rounded(t.radius.control)
		:border(1, u:focus_visible(id) and t.ring or t.clear)
		:child(shown)
end

local function waiting(alt, src)
	local t = u.theme
	return b.div()
		:row()
		:items_center()
		:gap(8)
		:p(10)
		:rounded(t.radius.control)
		:bg(b.alpha("foreground", 0.05))
		:no_shrink()
		:child(kit.spinner(13, "dim"))
		:child(b.text(name_of(src, alt)):text_size(SIZE.small):color("dim"):wrap():min_w(0))
end

-- What draws a picture of the text (`image` of the markdown library): the picture where the program
-- has it, a quiet box while it is on its way, else why not and where it is. `room` is the points the text
-- has across.
function pictures.drawer(owner, room)
	return function(src, alt, title)
		Pc.count += 1
		local n = Pc.count
		local mine = Pc.answers[owner]
		local answer = mine ~= nil and mine[src] or nil
		-- Written as a picture (`![]()`), not as a link that stands alone (made a picture by `standalone`).
		local image = title ~= LINKED
		if answer == nil then
			return refused(n, owner, src, alt, { status = "refused", why = "not-linear" }, image)
		elseif answer.status == "loading" then
			return waiting(alt, src)
		elseif answer.status == "ready" and answer.pdf then
			return card(n, owner, src, alt, answer, image)
		elseif answer.status == "ready" then
			return drawn(n, owner, src, alt, answer, room)
		end
		return refused(n, owner, src, alt, answer, image)
	end
end

-- --- Large ----------------------------------------------------------------------------------------------

function pictures.is_open()
	return Pc.big ~= nil
end

-- A frame number of the file of the element `id`, and what the page holds of it: the answer of the
-- program, ready.
local function ready_of(id)
	local one = Pc.ids[id]
	local mine = one ~= nil and Pc.answers[one.owner] or nil
	local answer = mine ~= nil and mine[one.src] or nil
	if answer == nil or answer.status ~= "ready" then
		return nil, nil
	end
	return one, answer
end

-- The picture of the element `id`, or the PDF of the card `id`, is opened over the pane. The reply.
function pictures.open(id)
	local one, answer = ready_of(id)
	if one == nil or (answer.pdf and not pictures.draws_pdf) then
		return true
	end
	Pc.big = {
		owner = one.owner,
		src = one.src,
		alt = one.alt,
		id = id,
		name = answer.name,
		width = answer.width,
		height = answer.height,
		pdf = answer.pdf,
		pages = answer.pages,
		page = 1,
	}
	if answer.pdf then
		-- The program reads it too, for its words (to select, to copy and to find) and the size of each page.
		pdf.begin(Pc.big, function(n)
			return pictures.go_to(n)
		end)
	end
	-- Where the issue was read to, so that it is where it was when the picture is put away.
	Pc.scroll = act.panel_scroll()
	return true
end

-- The link of a file or of a picture that is not Linear's: its address opens in the browser (only the
-- web's addresses do: the page never opens another kind, and the program fetches nothing). The reply.
function pictures.link(id)
	local one = Pc.ids[id]
	if one == nil or not is_web(one.src) then
		return true
	end
	return { link = one.src }
end

-- The card, or the card of the link, that a name of the frame belongs to: nil for another name.
local function card_of(id)
	local n = string.match(id or "", "^file%-card:(%d+)$")
		or string.match(id or "", "^file%-open:(%d+)$")
		or string.match(id or "", "^file%-save:(%d+)$")
		or string.match(id or "", "^pic%-link:(%d+)$")
		or string.match(id or "", "^pic%-ext:(%d+)$")
	for _, one in Pc.cards do
		if tostring(one.n) == n then
			return one
		end
	end
	return nil
end

-- A press on a card as a whole: its one verb (Open a PDF, Save any other file). The reply.
function pictures.card_press(id)
	local one = card_of(id)
	if one == nil then
		return true
	elseif one.ids.open ~= nil then
		return pictures.open(one.ids.open)
	end
	return pictures.save(one.ids.save)
end

-- Whether a PDF is large, and which page of it: the keys that turn the page are the page's.
function pictures.paging()
	return Pc.big ~= nil and Pc.big.pdf == true
end

local function page_to(n)
	local big = Pc.big
	if big == nil or not big.pdf then
		return true
	end
	local most = big.pages or math.huge
	n = math.max(1, math.min(n, most))
	if n ~= big.page then
		big.page = n
		pdf.went_to(big, n)
	end
	return true
end

-- The PDF that is large is turned to page `n`.
function pictures.go_to(n)
	return page_to(n)
end

-- The next or the previous page of the PDF that is large. The reply.
function pictures.turn(step)
	if Pc.big == nil then
		return true
	end
	return page_to(Pc.big.page + step)
end

-- The file is put away, and the keyboard is on it in the text again, unless it was put away with the
-- pointer (`pointer`): a press leaves the keyboard the list's. The reply.
function pictures.close(pointer)
	local big = Pc.big
	Pc.big = nil
	if big == nil then
		return true
	end
	if big.pdf then
		pdf.finish(big)
	end
	if Pc.scroll ~= nil then
		act.panel_scroll(Pc.scroll)
		Pc.scroll = nil
	end
	if pointer then
		return true
	end
	return u:focus(big.id)
end

-- A key while a file is large: Esc does the job of the button that says it (Done while the search is
-- open, else Close: what is selected stays, it is let go by a press elsewhere), and the arrows turn the
-- page of a PDF (with Shift they select the words). The reply, or nil.
function pictures.key(event)
	local big = Pc.big
	local mods = event.mods or {}
	if mods.command or mods.control or mods.alt then
		return nil
	end
	if event.key == "escape" and not mods.shift then
		if big ~= nil and pdf.find_open(big) then
			return pdf.close_find(big)
		end
		return pictures.close()
	elseif pictures.paging() and mods.shift then
		if event.key == "left" or event.key == "right" or event.key == "up" or event.key == "down" then
			return pdf.extend(big, event.key)
		end
	elseif pictures.paging() and event.key == "right" then
		return pictures.turn(1)
	elseif pictures.paging() and event.key == "left" then
		return pictures.turn(-1)
	end
	return nil
end

-- --- Saving ---------------------------------------------------------------------------------------------

-- The name a file is proposed to be saved by: the one it has, cleaned of what a path is made of, with `.pdf`
-- after a PDF's that has none.
local function save_name(src, alt, pdf)
	local name = string.gsub(name_of(src, alt, "file"), "[/\\%c]", "-")
	if pdf and kinds.extension(name) ~= "pdf" then
		name = name .. ".pdf"
	end
	return name
end

-- Save, on the card (or the large PDF) `id`: the Save panel is asked for, and what the person chooses
-- comes as `saving` (`pictures.saving`). The reply.
function pictures.save(id)
	local one = Pc.ids[id]
	if one == nil or Pc.saving[one.src] then
		return true
	end
	local mine = Pc.answers[one.owner]
	local answer = mine ~= nil and mine[one.src] or nil
	local pdf = answer ~= nil and answer.pdf == true
	Pc.save = { src = one.src, name = save_name(one.src, one.alt, pdf) }
	return { save = pdf and { name = Pc.save.name, type = "application/pdf" } or { name = Pc.save.name } }
end

-- The terminal says where the file is to be saved (`handle`; none where the person turned it away): the
-- program is asked to write it there. The reply.
function pictures.saving(event)
	local asked = Pc.save
	Pc.save = nil
	if asked == nil or event.handle == nil then
		return true
	end
	Pc.saving[asked.src] = true
	kit.call("save", { src = asked.src, handle = event.handle }, function(result, err)
		Pc.saving[asked.src] = nil
		if err then
			kit.toast("Could not save " .. asked.name .. ": " .. (err.message or err.code), "warn")
		else
			kit.toast("Saved " .. (event.name or asked.name) .. " (" .. kinds.size_words(result.size or 0) .. ")")
		end
		return true
	end)
	return true
end

-- The card the file commands act on (see `target_in`).
function pictures.target()
	if Pc.big ~= nil then
		return nil
	end
	return target_in(Pc.cards)
end

-- A press elsewhere: the card the menu was opened on is not the target any more.
function pictures.unpoint()
	Pc.pointed = nil
end

-- Whether Save can be done by key now: a card of a file to do it on (or the PDF that is large), not busy.
function pictures.saveable()
	if Pc.big ~= nil then
		return Pc.big.pdf == true and not Pc.saving[Pc.big.src]
	end
	local card = pictures.target()
	if card == nil or card.kind ~= "file" then
		return false
	end
	local one = Pc.ids[card.ids.save]
	return one ~= nil and not Pc.saving[one.src]
end

-- Whether Open (a PDF card) and the two that have an address (any card, or a link) can be done by key now.
function pictures.openable()
	local card = pictures.target()
	return card ~= nil and card.pdf == true
end

function pictures.linkable()
	return pictures.target() ~= nil
end

function pictures.save_target()
	if Pc.big ~= nil then
		return Pc.big.pdf and pictures.save("pdf-save") or true
	end
	local card = pictures.target()
	return card ~= nil and card.kind == "file" and pictures.save(card.ids.save) or true
end

function pictures.open_target()
	local card = pictures.target()
	return card ~= nil and card.ids.open ~= nil and pictures.open(card.ids.open) or true
end

function pictures.link_target()
	local card = pictures.target()
	return card ~= nil and pictures.link(card.ids.link) or true
end

-- The address of the card `id` is put on the pasteboard, and it is said so. The reply.
local function copy_link(id)
	local one = Pc.ids[id]
	if one == nil then
		return true
	end
	kit.toast("Copied the link", "info", 2.4)
	return { copy = one.src }
end

function pictures.copy_link_target()
	local card = pictures.target()
	return card ~= nil and copy_link(card.ids.link) or true
end

-- --- The menu of a card ----------------------------------------------------------------------------------

local MENU = "file-menu"

-- The menu that is open: its serial (a pick of an older menu is not one) and the address of its card.
local M = { serial = 0, src = nil }

local function entry(key, title, command, enabled)
	return { key = M.serial .. "|" .. key, title = title, keys = commands.first(command), enabled = enabled }
end

-- A right press on a card (or the link of a picture): its menu, at the place of the press; the card is the
-- one the file keys are for (so that the keys the menu says are the card's) until a press elsewhere. The
-- reply, or nil where the press was not on one.
function pictures.menu(event)
	if (event.button or "left") ~= "right" or Pc.big ~= nil then
		return nil
	end
	local card = card_of(b.layout.hit({ x = event.x, y = event.y }))
	local one = card ~= nil and Pc.ids[card.ids.link] or nil
	if one == nil then
		return nil
	end
	M.serial += 1
	M.src = one.src
	Pc.pointed = one.src
	local items = {}
	if card.ids.open ~= nil then
		items[#items + 1] = entry("open", "Open", "file_open")
	end
	if card.kind == "file" then
		items[#items + 1] = entry("save", "Save", "file_save", not Pc.saving[one.src])
	end
	items[#items + 1] = entry("link", "Open the link", "file_link", is_web(one.src))
	items[#items + 1] = entry("copy", "Copy the file's link", "file_copy_link")
	return u:menu(MENU, items, { at = { x = event.x, y = event.y } }) or true
end

-- What a pick of the menu of a card chose. The reply, or nil where it was not the menu's.
local function picked(action)
	if action.from ~= MENU or action.kind ~= "menu" then
		return nil
	end
	local serial, key = string.match(action.entry or "", "^(%d+)|(.+)$")
	if serial == nil or tonumber(serial) ~= M.serial then
		return true
	end
	for _, card in Pc.cards do
		local one = Pc.ids[card.ids.link]
		if one ~= nil and one.src == M.src then
			if key == "open" and card.ids.open ~= nil then
				return pictures.open(card.ids.open)
			elseif key == "save" and card.kind == "file" then
				return pictures.save(card.ids.save)
			elseif key == "link" then
				return pictures.link(card.ids.link)
			elseif key == "copy" then
				return copy_link(card.ids.link)
			end
		end
	end
	return true
end

-- --- What the words of a PDF are asked of --------------------------------------------------------------

-- Whether the PDF that is large has words to select and to find.
function pictures.readable()
	return pdf.readable(Pc.big)
end

function pictures.selected()
	return pdf.selected(Pc.big)
end

function pictures.found()
	return pdf.found(Pc.big)
end

function pictures.find_open()
	return pdf.find_open(Pc.big)
end

-- The search is asked for: the bar is up and the keyboard is in its field. The reply.
function pictures.find()
	return pdf.open_find(Pc.big)
end

-- The match after (1) or before (-1). The reply.
function pictures.step(direction)
	return pdf.step(Pc.big, direction)
end

function pictures.close_find()
	return pdf.close_find(Pc.big)
end

-- The selection is copied. The reply.
function pictures.copy()
	return pdf.copy(Pc.big) or true
end

-- All the words of the page are selected. The reply.
function pictures.select_all()
	return pdf.select_all(Pc.big) or true
end

-- What the field of the search does. The reply, or nil.
function pictures.action(action)
	local chosen = picked(action)
	if chosen ~= nil then
		return chosen
	end
	if Pc.big == nil then
		return nil
	end
	return pdf.action(Pc.big, action)
end

-- The pointer on the page of a PDF that is large: a press, the pointer moving, the button going up, and
-- whether it is over words. The reply, or nil.
function pictures.pressed(event)
	return Pc.big ~= nil and pdf.pressed(Pc.big, event) or nil
end

function pictures.dragged(event)
	return pdf.dragged(Pc.big, event)
end

function pictures.released()
	return pdf.released(Pc.big)
end

function pictures.over_text(event)
	return Pc.big ~= nil and pdf.over_text(Pc.big, event)
end

-- --- The file large ---------------------------------------------------------------------------------------

local function title_row(big)
	local row = b.div():row():items_center():gap(8):no_shrink():px(SIZE.pad):pt(SIZE.head_top):pb(10)
	row:child(
		b.text(name_of(big.src, big.alt)):text_size(SIZE.body):weight(700):truncate():min_w(0):grow(1)
	)
	-- Esc is written on the one button it does: Done while the search is open, else this. (This is the
	-- last of its row, so that its words going do not move anything else.)
	local searching = big.pdf and pdf.find_open(big)
	row:child(kit.button("picture-close", "Close", { suffix = (not searching) and "Esc" or nil }))
	return row
end

-- The row of a PDF's pages: which one is shown, the way to the page before and the one after, and, at the
-- other end, the search (where there are words to find) and Save. Each button has its key, always (nothing
-- of this row moves as the search is opened).
local function pages_row(big)
	local row = b.div():row():items_center():gap(8):no_shrink():px(SIZE.pad):pb(10)
	local words = big.pages ~= nil and ("Page " .. big.page .. " of " .. big.pages) or ("Page " .. big.page)
	row:child(b.text(words):text_size(SIZE.small):color("dim"):no_shrink())
	row:child(kit.button("pdf-previous", "Previous", {
		suffix = commands.label("pdf_previous"),
		disabled = big.page <= 1,
		tip = "The page before",
	}))
	row:child(kit.button("pdf-next", "Next", {
		suffix = commands.label("pdf_next"),
		disabled = big.pages ~= nil and big.page >= big.pages,
		tip = "The page after",
	}))
	row:child(b.div():grow(1):min_w(0))
	if pdf.readable(big) then
		row:child(kit.button("pdf-find-open", "Find", { suffix = commands.label("pdf_find"), tip = "Find words in the PDF" }))
	end
	row:child(kit.button("pdf-save", "Save", {
		suffix = commands.label("file_save"),
		busy = Pc.saving[big.src] == true,
		disabled = Pc.saving[big.src] == true,
		tip = "Save the PDF in a place you choose",
	}))
	Pc.ids["pdf-save"] = { owner = big.owner, src = big.src, alt = big.alt }
	return row
end

-- The picture or the PDF large, as the room of the page shows it: its name, Save (a PDF) and Close at the
-- top, for a PDF the page it is at and the buttons that turn it and begin the search, and the page fitted
-- in a box that scrolls and that the terminal zooms (pinched, or by the wheel with the command key), where
-- its kit does. A PDF's page has the marks of what was found and what is selected laid over it.
function pictures.view()
	local big = Pc.big
	local top = b.div():col():no_shrink():child(title_row(big))
	if big.pdf then
		top:child(pages_row(big))
		if pdf.find_open(big) then
			top:child(pdf.find_bar(big))
		end
	end
	local room_w = math.max(S.width - 2 * SIZE.pad - 8, 40)
	local room_h = math.max(S.height - (big.pdf and 190 or 140) - (pdf.find_open(big) and 48 or 0), 40)
	local width, height = big.width, big.height
	if big.pdf then
		width, height = pdf.size_of(big, big.page)
	end
	local w, h = fitted(width, height, room_w, room_h)
	local f = fit(width, height, room_w, room_h)
	-- Large as the picture's own size, and not past what four times the fit comes to.
	local most = math.max(math.min(4, 1 / f * 2), 1)
	local shown = b.image(big.name):size(w, h):fit("contain"):no_shrink():role("image"):label(name_of(big.src, big.alt))
	local held
	if big.pdf then
		shown:page(big.page)
		-- The page and what is laid over it are one box, as large as the page is drawn.
		local sheet = b.div():id("pdf-page"):size(w, h):no_shrink():bg("#ffffff"):child(shown)
		for _, element in pdf.layer(big, big.page, w, h) do
			sheet:child(element)
		end
		held = b.div():col():items_center():justify_center():p(8):child(sheet)
	else
		held = b.div():col():items_center():justify_center():p(8):child(shown)
	end
	local box = b.div():id("picture-zoom"):grid({ "auto" }):grid_rows({ "auto" }):grow(1):min_w(0):min_h(0):scroll()
	kit.zoom(box, { min = 1, max = most, smart = math.min(2, most) })
	box:child(held)
	return b.div():col():grow(1):min_w(0):min_h(0):child(top):child(box)
end

return pictures
