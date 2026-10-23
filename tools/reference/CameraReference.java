import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.ClipContext;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.Fluids;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.phys.HitResult;
public class CameraReference {
 public static void main(String[] args) {
  SharedConstants.tryDetectVersion();Bootstrap.bootStrap();
  BlockGetter world=new BlockGetter(){
   public BlockEntity getBlockEntity(BlockPos p){return null;}
   public BlockState getBlockState(BlockPos p){return p.getZ()>=2?Blocks.STONE.defaultBlockState():Blocks.AIR.defaultBlockState();}
   public FluidState getFluidState(BlockPos p){return Fluids.EMPTY.defaultFluidState();}
   public int getHeight(){return 384;}
   public int getMinY(){return -64;}
  };
  Vec3 eye=new Vec3(0,1.62,0);float distance=4;
  for(int i=0;i<8;i++){
   Vec3 from=eye.add(((i&1)*2-1)*0.1f,(((i>>1)&1)*2-1)*0.1f,(((i>>2)&1)*2-1)*0.1f);
   var hit=world.clip(new ClipContext(from,from.add(0,0,distance),ClipContext.Block.VISUAL,ClipContext.Fluid.NONE,net.minecraft.world.phys.shapes.CollisionContext.empty()));
   if(hit.getType()!=HitResult.Type.MISS){float d=(float)hit.getLocation().distanceToSqr(eye);if(d<distance*distance)distance=(float)Math.sqrt(d);}
  }
  System.out.println("Original visual clip zoom="+distance);
 }
}
