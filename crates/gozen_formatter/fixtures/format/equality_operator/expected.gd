extends Node

var damage = 0 if get_rank() == 0 else Upgrades.Damage[get_rank()-1]
var speed = 0 if get_rank() == 0 else Upgrades.Speed[get_rank() - 1]
var active = true if count != 0 else false
var valid = true if score >= 50 else false
var low = true if score <= 10 else false
