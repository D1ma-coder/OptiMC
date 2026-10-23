import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.core.registries.*;
import net.minecraft.core.component.*;
import net.minecraft.network.*;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.game.*;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.*;
import net.minecraft.world.item.component.*;
import net.minecraft.world.inventory.MenuType;
import net.minecraft.world.level.block.Blocks;
import io.netty.buffer.Unpooled;
public class ExportInventory {
 static RegistryFriendlyByteBuf buffer(){return new RegistryFriendlyByteBuf(Unpooled.buffer(),RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY));}
 static <T> String encode(StreamCodec<? super RegistryFriendlyByteBuf,T> codec,T value){var b=buffer();try{codec.encode(b,value);var data=new byte[b.readableBytes()];b.readBytes(data);return Base64.getEncoder().encodeToString(data);}finally{b.release();}}
 static <T> void packet(JsonArray rows,String label,int id,StreamCodec<? super RegistryFriendlyByteBuf,T> codec,T value){var row=new JsonObject();row.addProperty("label",label);row.addProperty("id",id);row.addProperty("body",encode(codec,value));rows.add(row);}
 public static void main(String[] args)throws Exception{
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();var dynamicLookup=net.minecraft.data.registries.VanillaRegistries.createLookup();var staticLookup=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);var keys=staticLookup.listRegistryKeys().collect(java.util.stream.Collectors.toSet());var lookup=HolderLookup.Provider.create(java.util.stream.Stream.concat(staticLookup.listRegistries(),dynamicLookup.listRegistries().filter(r->!keys.contains(r.key()))));BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(lookup).forEach(DataComponentInitializers.PendingComponents::apply);var defaults=new JsonArray();
  for(var item:BuiltInRegistries.ITEM){var row=new JsonObject();row.addProperty("id",BuiltInRegistries.ITEM.getId(item));row.addProperty("name",BuiltInRegistries.ITEM.getKey(item).toString());row.addProperty("translation",item.getDescriptionId());row.addProperty("maximum",item.getDefaultMaxStackSize());var components=new JsonObject();var errors=new JsonArray();
   for(var component:item.components())try{var type=(DataComponentType)component.type();if(type.streamCodec()!=null)components.addProperty(BuiltInRegistries.DATA_COMPONENT_TYPE.getId(type)+"",encode(type.streamCodec(),component.value()));}catch(Throwable e){errors.add(component.type()+": "+e.toString());}
   var values=new JsonObject();var jsonErrors=new JsonArray();for(var c:item.components())try{var type=(DataComponentType)c.type();if(type.codec()!=null)values.add(BuiltInRegistries.DATA_COMPONENT_TYPE.getKey(type).toString(),(JsonElement)type.codec().encodeStart(net.minecraft.resources.RegistryOps.create(com.mojang.serialization.JsonOps.INSTANCE,lookup),c.value()).getOrThrow());}catch(Throwable e){jsonErrors.add(c.type()+": "+e.toString());}row.add("values",values);row.add("json_errors",jsonErrors);row.add("components",components);row.add("errors",errors);defaults.add(row);
  }
  var packets=new JsonArray();var tool=new ItemStack(Items.IRON_PICKAXE);tool.set(DataComponents.DAMAGE,7);tool.set(DataComponents.CUSTOM_NAME,Component.literal("Серверная кирка").withStyle(net.minecraft.ChatFormatting.GOLD));
  tool.set(DataComponents.TOOL,new Tool(List.of(Tool.Rule.minesAndDrops(HolderSet.direct(Blocks.STONE.builtInRegistryHolder()),12.5f)),1f,1,true));tool.set(DataComponents.ATTACK_RANGE,new AttackRange(1,5,1,6,.25f,1.5f));tool.set(DataComponents.ENCHANTMENTS,net.minecraft.world.item.enchantment.ItemEnchantments.EMPTY);
  var items=NonNullList.withSize(46,ItemStack.EMPTY);items.set(36,tool);items.set(37,new ItemStack(Items.APPLE,12));items.set(9,new ItemStack(Items.BREAD,4));items.set(5,new ItemStack(Items.DIAMOND_HELMET));items.set(45,new ItemStack(Items.TORCH,32));
  packet(packets,"player_content",0x12,ClientboundContainerSetContentPacket.STREAM_CODEC,new ClientboundContainerSetContentPacket(0,5,items,ItemStack.EMPTY));
  packet(packets,"player_slot",0x14,ClientboundContainerSetSlotPacket.STREAM_CODEC,new ClientboundContainerSetSlotPacket(0,6,37,new ItemStack(Items.APPLE,9)));
  packet(packets,"direct_player_slot",0x6c,ClientboundSetPlayerInventoryPacket.STREAM_CODEC,new ClientboundSetPlayerInventoryPacket(8,new ItemStack(Items.DIAMOND,3)));
  packet(packets,"cursor",0x60,ClientboundSetCursorItemPacket.STREAM_CODEC,new ClientboundSetCursorItemPacket(new ItemStack(Items.BOOK,2)));
  packet(packets,"open_chest",0x3b,ClientboundOpenScreenPacket.STREAM_CODEC,new ClientboundOpenScreenPacket(300,MenuType.GENERIC_9x3,Component.literal("Серверный сундук")));
  var chest=NonNullList.withSize(63,ItemStack.EMPTY);chest.set(0,new ItemStack(Items.DIAMOND,5));chest.set(54,tool);
  packet(packets,"chest_content",0x12,ClientboundContainerSetContentPacket.STREAM_CODEC,new ClientboundContainerSetContentPacket(300,7,chest,ItemStack.EMPTY));
  packet(packets,"chest_slot",0x14,ClientboundContainerSetSlotPacket.STREAM_CODEC,new ClientboundContainerSetSlotPacket(300,8,0,new ItemStack(Items.DIAMOND,6)));
  packet(packets,"stale_slot",0x14,ClientboundContainerSetSlotPacket.STREAM_CODEC,new ClientboundContainerSetSlotPacket(301,9,0,new ItemStack(Items.DIRT,64)));
  packet(packets,"data",0x13,ClientboundContainerSetDataPacket.STREAM_CODEC,new ClientboundContainerSetDataPacket(300,2,42));
  packet(packets,"close",0x11,ClientboundContainerClosePacket.STREAM_CODEC,new ClientboundContainerClosePacket(300));
  var layouts=new JsonArray();
  var uf=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");uf.setAccessible(true);var unsafe=(sun.misc.Unsafe)uf.get(null);
  var player=(net.minecraft.world.entity.player.Player)unsafe.allocateInstance(net.minecraft.client.player.RemotePlayer.class);
  var inventory=new net.minecraft.world.entity.player.Inventory(player,new net.minecraft.world.entity.EntityEquipment());
  for(var type:BuiltInRegistries.MENU){var row=new JsonObject();row.addProperty("id",BuiltInRegistries.MENU.getId(type));row.addProperty("name",BuiltInRegistries.MENU.getKey(type).toString());
   try{var menu=type.create(300,inventory);var slots=new JsonArray();for(var slot:menu.slots){var v=new JsonArray();v.add(slot.x);v.add(slot.y);v.add(slot.container==inventory?slot.getContainerSlot():-1);slots.add(v);}row.add("slots",slots);}catch(Throwable err){row.addProperty("error",err.toString());}layouts.add(row);
  }
  var actions=new JsonArray();
  for(var action:List.of(ServerboundPlayerActionPacket.Action.SWAP_ITEM_WITH_OFFHAND,ServerboundPlayerActionPacket.Action.DROP_ITEM,ServerboundPlayerActionPacket.Action.DROP_ALL_ITEMS))packet(actions,action.name(),0x29,ServerboundPlayerActionPacket.STREAM_CODEC,new ServerboundPlayerActionPacket(action,BlockPos.ZERO,Direction.DOWN));
  packet(actions,"close_300",0x13,ServerboundContainerClosePacket.STREAM_CODEC,new ServerboundContainerClosePacket(300));packet(actions,"close_0",0x13,ServerboundContainerClosePacket.STREAM_CODEC,new ServerboundContainerClosePacket(0));
  var unsupported=new JsonArray();var model=new ItemStack(Items.STICK);model.set(DataComponents.BEES,new Bees(List.of()));packet(unsupported,"bees",0x60,ClientboundSetCursorItemPacket.STREAM_CODEC,new ClientboundSetCursorItemPacket(model));
  var root=new JsonObject();root.add("actions",actions);root.add("unsupported",unsupported);root.add("layouts",layouts);root.add("defaults",defaults);root.add("packets",packets);Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
