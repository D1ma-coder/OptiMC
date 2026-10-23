// CPU-only direct calls into the downloaded 26.2 HumanoidModel.
import com.google.gson.*;import java.nio.file.*;
import net.minecraft.client.model.*;import net.minecraft.client.model.geom.*;import net.minecraft.client.model.geom.builders.*;
import net.minecraft.client.renderer.entity.state.HumanoidRenderState;import net.minecraft.world.entity.HumanoidArm;import net.minecraft.world.item.SwingAnimationType;
public class ExportHumanoidAttack {
 static class Probe extends HumanoidModel<HumanoidRenderState> {
  Probe(){super(LayerDefinition.create(HumanoidModel.createMesh(CubeDeformation.NONE,0),64,64).bakeRoot());}
  void attack(HumanoidRenderState state){setupAttackAnimation(state);}
 }
 static JsonArray parts(Probe m){var result=new JsonArray();for(var p:new ModelPart[]{m.head,m.body,m.rightArm,m.leftArm}){var a=new JsonArray();for(float f:new float[]{p.x,p.y,p.z,p.xRot,p.yRot,p.zRot,p.xScale,p.yScale,p.zScale})a.add(f);result.add(a);}return result;}
 public static void main(String[] args)throws Exception {
  net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
  var rows=new JsonArray();
  for(boolean left:new boolean[]{false,true})for(float pitch:new float[]{-1.2f,0,0.7f,1.2f})for(float scale:new float[]{0.5f,1})for(int step=0;step<=16;step++) {
   var m=new Probe();m.head.xRot=pitch;var s=new HumanoidRenderState();s.attackTime=step/16f;s.attackArm=left?HumanoidArm.LEFT:HumanoidArm.RIGHT;s.ageScale=scale;s.swingAnimationType=SwingAnimationType.WHACK;
   var row=new JsonObject();row.addProperty("left",left);row.addProperty("pitch",pitch);row.addProperty("scale",scale);row.addProperty("progress",s.attackTime);row.add("initial",parts(m));m.attack(s);row.add("expected",parts(m));rows.add(row);
  }
  var root=new JsonObject();root.add("poses",rows);
  var metadata=new JsonObject();
  for(var entry:new Object[][]{{net.minecraft.world.entity.Mob.class,"DATA_MOB_FLAGS_ID"},{net.minecraft.world.entity.Avatar.class,"DATA_PLAYER_MAIN_HAND"}}){var f=((Class<?>)entry[0]).getDeclaredField((String)entry[1]);f.setAccessible(true);metadata.addProperty((String)entry[1],((net.minecraft.network.syncher.EntityDataAccessor<?>)f.get(null)).id());}
  root.add("metadata",metadata);
  var packets=new JsonArray();
  for(int id:new int[]{0,300,40960,-1})for(int action:new int[]{0,2,3,4,5,255}) {
   var buf=new net.minecraft.network.FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());buf.writeVarInt(id);buf.writeByte(action);var packet=net.minecraft.network.protocol.game.ClientboundAnimatePacket.STREAM_CODEC.decode(buf);buf.clear();net.minecraft.network.protocol.game.ClientboundAnimatePacket.STREAM_CODEC.encode(buf,packet);byte[] bytes=new byte[buf.readableBytes()];buf.readBytes(bytes);buf.release();var row=new JsonObject();row.addProperty("entity",packet.getId());row.addProperty("action",packet.getAction());row.addProperty("body",java.util.Base64.getEncoder().encodeToString(bytes));packets.add(row);
  }
  root.add("packets",packets);
  Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
