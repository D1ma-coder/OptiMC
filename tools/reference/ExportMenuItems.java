// CPU-only fixtures: plugin menu items must not discard a whole content packet.
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.*;
import net.minecraft.core.registries.*;
import net.minecraft.core.component.*;
import net.minecraft.network.*;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.game.*;
import net.minecraft.world.item.*;
import net.minecraft.world.item.component.*;
import net.minecraft.world.item.alchemy.*;
import net.minecraft.world.effect.*;
import com.mojang.authlib.GameProfile;
public class ExportMenuItems {
 public static void main(String[] args)throws Exception{
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  var dl=net.minecraft.data.registries.VanillaRegistries.createLookup();var sl=RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
  var keys=sl.listRegistryKeys().collect(java.util.stream.Collectors.toSet());var lookup=HolderLookup.Provider.create(java.util.stream.Stream.concat(sl.listRegistries(),dl.listRegistries().filter(r->!keys.contains(r.key()))));
  BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(lookup).forEach(DataComponentInitializers.PendingComponents::apply);
  var stacks=new ArrayList<ItemStack>();
  var model=new ItemStack(Items.COMPASS);model.set(DataComponents.CUSTOM_MODEL_DATA,new CustomModelData(List.of(1.25f,-2f),List.of(true,false),List.of("server","Выбор режима"),List.of(0xffaabb,0)));stacks.add(model);
  var head=new ItemStack(Items.PLAYER_HEAD);head.set(DataComponents.PROFILE,ResolvableProfile.createResolved(new GameProfile(new UUID(42,12),"TestPlayer")));stacks.add(head);
  var named=new ItemStack(Items.PLAYER_HEAD);named.set(DataComponents.PROFILE,ResolvableProfile.createUnresolved("TestPlayer"));stacks.add(named);
  var uuid=new ItemStack(Items.PLAYER_HEAD);uuid.set(DataComponents.PROFILE,ResolvableProfile.createUnresolved(new UUID(12,42)));stacks.add(uuid);
  var potion=new ItemStack(Items.POTION);potion.set(DataComponents.POTION_CONTENTS,new PotionContents(Optional.of(Potions.HEALING),Optional.of(0xabcdef),List.of(new MobEffectInstance(MobEffects.SPEED,120,2,false,true,false)),Optional.of("menu_potion")));potion.set(DataComponents.POTION_DURATION_SCALE,1.25f);stacks.add(potion);
  var tag=new net.minecraft.nbt.CompoundTag();tag.putString("CustomName","Меню");
  var stand=new ItemStack(Items.ARMOR_STAND);stand.set(DataComponents.ENTITY_DATA,TypedEntityData.of(BuiltInRegistries.ENTITY_TYPE.getValue(net.minecraft.resources.Identifier.parse("minecraft:armor_stand")),tag));stacks.add(stand);
  var chest=new ItemStack(Items.CHEST);chest.set(DataComponents.BLOCK_ENTITY_DATA,TypedEntityData.of(BuiltInRegistries.BLOCK_ENTITY_TYPE.getValue(net.minecraft.resources.Identifier.parse("minecraft:chest")),tag));stacks.add(chest);
  var bucket=new ItemStack(Items.COD_BUCKET);bucket.set(DataComponents.BUCKET_ENTITY_DATA,CustomData.of(tag));stacks.add(bucket);
  var rows=new JsonArray();for(int i=0;i<stacks.size();i++){var stack=stacks.get(i);ExportItemContents.fixture(rows,"menu_item_"+i,stack);}
  var packets=new JsonArray();ExportInventory.packet(packets,"open",0x3b,ClientboundOpenScreenPacket.STREAM_CODEC,new ClientboundOpenScreenPacket(42,net.minecraft.world.inventory.MenuType.GENERIC_9x3,Component.literal("Выбери режим")));
  var slots=NonNullList.withSize(63,ItemStack.EMPTY);for(int i=0;i<stacks.size();i++)slots.set(i,stacks.get(i));slots.set(54,new ItemStack(Items.COMPASS));ExportInventory.packet(packets,"content",0x12,ClientboundContainerSetContentPacket.STREAM_CODEC,new ClientboundContainerSetContentPacket(42,7,slots,ItemStack.EMPTY));
  var root=new JsonObject();root.add("stacks",rows);root.add("packets",packets);Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
