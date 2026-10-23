import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.protocol.game.ClientboundUpdateAttributesPacket;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.attributes.*;
import io.netty.buffer.Unpooled;
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;
public class ExportAttributes {
 public static void main(String[] args) throws Exception {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  var root=new JsonObject();root.addProperty("minecraft","26.2");root.addProperty("protocol",776);var entries=new JsonArray();
  for(var a:BuiltInRegistries.ATTRIBUTE){var entry=new JsonObject();entry.addProperty("id",BuiltInRegistries.ATTRIBUTE.getId(a));entry.addProperty("name",BuiltInRegistries.ATTRIBUTE.getKey(a).toString());entry.addProperty("default",a.getDefaultValue());if(a instanceof RangedAttribute r){entry.addProperty("min",r.getMinValue());entry.addProperty("max",r.getMaxValue());}entry.addProperty("syncable",a.isClientSyncable());entries.add(entry);}
  root.add("attributes",entries);Files.writeString(Path.of(args[0],"attributes.json"),new Gson().toJson(root));
  var instance=new AttributeInstance(Attributes.GRAVITY,a->{});instance.setBaseValue(0.08);
  instance.addTransientModifier(new AttributeModifier(Identifier.withDefaultNamespace("test_add"),0.02,AttributeModifier.Operation.ADD_VALUE));
  instance.addTransientModifier(new AttributeModifier(Identifier.withDefaultNamespace("test_base"),0.5,AttributeModifier.Operation.ADD_MULTIPLIED_BASE));
  instance.addTransientModifier(new AttributeModifier(Identifier.withDefaultNamespace("test_total"),1.0,AttributeModifier.Operation.ADD_MULTIPLIED_TOTAL));
  var packet=new ClientboundUpdateAttributesPacket(42,List.of(instance));
  var buffer=new RegistryFriendlyByteBuf(Unpooled.buffer(),RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY));
  ClientboundUpdateAttributesPacket.STREAM_CODEC.encode(buffer,packet);byte[] bytes=new byte[buffer.readableBytes()];buffer.readBytes(bytes);Files.write(Path.of(args[0],"attribute-packet.bin"),bytes);buffer.release();
  var golden=new JsonObject();golden.addProperty("modified_gravity",instance.getValue());var falling=new JsonArray();
  var friction=LivingEntity.class.getDeclaredMethod("computeModifiedFriction",float.class,float.class);friction.setAccessible(true);
  double y=80,vy=0;float drag=(Float)friction.invoke(null,0.98f,1.0f);
  for(int tick=1;tick<=120;tick++){if(Math.abs(vy)<0.003)vy=0;y+=vy;vy=(vy-0.08)*(double)drag;var frame=new JsonObject();frame.addProperty("tick",tick);frame.addProperty("y",y);frame.addProperty("vy",vy);frame.addProperty("y_bits",Long.toUnsignedString(Double.doubleToRawLongBits(y),16));frame.addProperty("vy_bits",Long.toUnsignedString(Double.doubleToRawLongBits(vy),16));falling.add(frame);}
  golden.add("empty_world_fall",falling);Files.writeString(Path.of(args[0],"physics-golden.json"),new Gson().toJson(golden));
  System.out.println("Exported "+entries.size()+" attributes; original packet bytes="+bytes.length+"; modified gravity="+instance.getValue()+"; drag="+Double.toString((double)drag));
 }
}
