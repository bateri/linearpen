-- The card that chooses several values in one go (the labels of an issue, the values of a filter): the
-- kit's picker, which stays open while the person turns one value after another on and off, with a field
-- that narrows what it offers. A terminal whose kit has no picker (`u.picker`, rules/penler.md) keeps the
-- host's menu, which closes at every pick: the part that opens a card asks `picker.available()` first.
--
-- One card at a time, by the control that opened it. The part that opens it says what it offers and what
-- a choice does; this one draws it last on the page (it is the layer in front, which holds the keyboard
-- and the commands), tells it what the person did, and closes it where its control is gone.

local kit = require("../lib/kit")

local b, u = kit.b, kit.u

local picker = {}

local ID = "values-picker"

-- The card open, or nil: `anchor` (the name of the control it stands by), `label` and `hint` (its words),
-- `items()` (what it offers now, each `{ key, title, icon, on }`, nil where there is nothing to choose
-- any more: the issues or the choices are gone), `empty()` (what it says where it offers nothing, nil
-- for the kit's words; where it is not given, the kit's), `toggle(item, on)`, `closed()` (told when it
-- goes), `state` (what is typed and the cursor, the kit's) and `drawn` (it has been on the page).
local P = nil

-- Whether the terminal's kit has a picker.
function picker.available()
	return u.picker ~= nil
end

function picker.is_open()
	return P ~= nil
end

-- What the control `anchor` shows of its values while its card is open: `value` as it was when the card
-- opened (the first that is asked), so that the control, which says the values it holds, does not grow
-- and wrap under the card at every turn and take the card with it; `value` itself where its card is not
-- open. The control says what was chosen when the card closes; the card says it meanwhile.
function picker.steady(anchor, value)
	if P == nil or P.anchor ~= anchor then
		return value
	end
	if P.steady == nil then
		P.steady = { value = value }
	end
	return P.steady.value
end

-- Whether the card open is the one by the control `anchor`.
function picker.by(anchor)
	return P ~= nil and P.anchor == anchor
end

-- Closes the card, where one is open.
function picker.close()
	local was = P
	P = nil
	if was ~= nil and was.closed ~= nil then
		was.closed()
	end
end

-- Opens the card `spec` (`anchor`, `label`, `hint`, `items`, `empty`, `toggle`, `closed`) by its
-- control: a second press of the control that opened the card closes it. False where the control is not
-- on the page (the part opens the host's menu then).
function picker.open(spec)
	if picker.by(spec.anchor) then
		picker.close()
		return true
	end
	if b.layout.bounds(spec.anchor) == nil then
		return false
	end
	picker.close()
	P = {
		anchor = spec.anchor,
		label = spec.label,
		hint = spec.hint,
		items = spec.items,
		empty = spec.empty,
		toggle = spec.toggle,
		closed = spec.closed,
		state = { query = "" },
		drawn = false,
	}
	return true
end

-- Draws the card on `root`, after everything else on the page. A card whose control was not laid out
-- (the pane narrowed, the row of the filters went) or that has nothing to offer any more closes.
function picker.view(root)
	if P == nil then
		return
	end
	local items = P.items()
	if items == nil or (P.drawn and b.layout.bounds(P.anchor) == nil) then
		picker.close()
		return
	end
	P.drawn = true
	root:child(u:picker(ID, {
		anchor = P.anchor,
		items = items,
		multi = true,
		state = P.state,
		label = P.label,
		hint = P.hint,
		empty = P.empty ~= nil and P.empty() or nil,
	}))
end

-- Whether the element `name` (what the pointer went down on) is in the card.
function picker.holds(name)
	if P == nil or name == nil then
		return false
	end
	for _, around in b.layout.path_of(name) do
		if around == ID then
			return true
		end
	end
	return false
end

-- What the person did to the card. The reply, or nil where it was not the card's.
function picker.action(action)
	if action.from ~= ID then
		return nil
	end
	if P == nil then
		return true
	end
	if action.kind == "toggle" then
		return P.toggle(action.item, action.on) or true
	elseif action.kind == "dismiss" then
		picker.close()
	end
	return true
end

return picker
